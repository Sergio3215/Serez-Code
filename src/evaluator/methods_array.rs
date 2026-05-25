#![allow(unused_imports)]
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use super::{EvalResult, StoredClass, CallFrame, type_matches, obj_data_to_key_str,
            obj_data_eq, format_decimal, json_stringify_owned, json_parse,
            operator_to_method_name};

impl super::Evaluator {
    pub(super) fn eval_array_method(
        &mut self,
        arr_ref: ObjectRef,
        element_type: Option<String>,
        elems: Vec<ObjectRef>,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {

            "length" => EvalResult::Value(self.alloc(ObjectData::Integer(elems.len() as i64))),

            "push" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: push expects 1 argument");
                    return EvalResult::Error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if let Some(ref et) = element_type {
                    let data = self.resolve(val_ref).unwrap();
                    if !type_matches(et, data) {
                        eprintln!(
                            "❌ TYPE ERROR: Cannot push '{}' into [{}] array",
                            data.type_name(), et
                        );
                        return EvalResult::Error;
                    }
                }
                let val = self.extract(val_ref);
                let new_ref = match arr_ref.region {
                    RegionId::Global => self.plant_global(val),
                    RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(val),
                    RegionId::Scoped => self.plant(val),
                };
                let mut e = elems;
                e.push(new_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "pop" => {
                if elems.is_empty() {
                    eprintln!("❌ ERROR: pop() called on an empty array");
                    return EvalResult::Error;
                }
                let mut e = elems;
                let last = e.pop().unwrap();
                let owned = self.extract(last);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "shift" => {
                if elems.is_empty() {
                    eprintln!("❌ ERROR: shift() called on an empty array");
                    return EvalResult::Error;
                }
                let mut e = elems;
                let first = e.remove(0);
                let owned = self.extract(first);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "unshift" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: unshift expects 1 argument");
                    return EvalResult::Error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if let Some(ref et) = element_type {
                    let data = self.resolve(val_ref).unwrap();
                    if !type_matches(et, data) {
                        eprintln!(
                            "❌ TYPE ERROR: Cannot unshift '{}' into [{}] array",
                            data.type_name(), et
                        );
                        return EvalResult::Error;
                    }
                }
                let val = self.extract(val_ref);
                let new_ref = match arr_ref.region {
                    RegionId::Global => self.plant_global(val),
                    RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(val),
                    RegionId::Scoped => self.plant(val),
                };
                let mut e = elems;
                e.insert(0, new_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "remove" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: remove expects 1 argument (index)");
                    return EvalResult::Error;
                }
                let idx = match self.eval_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => return EvalResult::Error,
                };
                if elems.is_empty() {
                    return EvalResult::Value(self.null_ref);
                }
                if idx < 0 || idx as usize >= elems.len() {
                    eprintln!("❌ ERROR: remove: index {} out of bounds (length {})", idx, elems.len());
                    return EvalResult::Error;
                }
                let mut e = elems;
                let removed_ref = e.remove(idx as usize);
                let owned = self.extract(removed_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "sort" => {
                // Evaluate the optional argument exactly once
                let arg_ref: Option<ObjectRef> = if dot_call.arguments.len() == 1 {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => Some(r),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                } else {
                    None
                };

                // If the argument is a function, use it as a comparator
                let is_comparator = arg_ref.map_or(false, |r| {
                    matches!(self.resolve(r), Some(ObjectData::Function { .. }))
                });

                if is_comparator {
                    let cb_ref = arg_ref.unwrap();
                    let mut owned_vals: Vec<OwnedValue> =
                        elems.iter().map(|&r| self.extract(r)).collect();
                    let n = owned_vals.len();
                    // Bubble sort (simple, avoids borrow issues with call_function)
                    let mut i = 0;
                    let mut sort_err: Option<EvalResult> = None;
                    'outer: while i < n {
                        let mut j = 0;
                        while j < n - i - 1 {
                            let a = owned_vals[j].clone();
                            let b = owned_vals[j + 1].clone();
                            let cmp_result = self.call_function(cb_ref, vec![a, b]);
                            let should_swap = match cmp_result {
                                EvalResult::Value(r) => match self.resolve(r) {
                                    Some(ObjectData::Integer(v)) => *v > 0,
                                    Some(ObjectData::Decimal(v)) => *v > 0.0,
                                    _ => {
                                        eprintln!("❌ ERROR: sort comparator must return a number");
                                        sort_err = Some(EvalResult::Error);
                                        break 'outer;
                                    }
                                },
                                EvalResult::Throw(v) => {
                                    sort_err = Some(EvalResult::Throw(v));
                                    break 'outer;
                                }
                                _ => {
                                    sort_err = Some(EvalResult::Error);
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
                    let new_refs: Vec<ObjectRef> = owned_vals.into_iter().map(|v| {
                        match arr_ref.region {
                            RegionId::Global => self.plant_global(v),
                            RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(v),
                            RegionId::Scoped => self.plant(v),
                        }
                    }).collect();
                    self.update_array(arr_ref, element_type, new_refs);
                    return EvalResult::Value(arr_ref);
                }

                let order = match arg_ref {
                    None => "asc".to_string(),
                    Some(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => "asc".to_string(),
                    },
                };
                let descending = order == "desc";

                let mut owned_vals: Vec<OwnedValue> =
                    elems.iter().map(|&r| self.extract(r)).collect();

                let all_ints = owned_vals.iter().all(|v| matches!(v, OwnedValue::Integer(_)));
                let all_decs = owned_vals.iter().all(|v| matches!(v, OwnedValue::Decimal(_)));
                let all_strs = owned_vals.iter().all(|v| matches!(v, OwnedValue::Str(_)));

                if !all_ints && !all_decs && !all_strs {
                    eprintln!("❌ ERROR: sort requires a homogeneous array (all int, decimal, or string)");
                    return EvalResult::Error;
                }

                owned_vals.sort_by(|a, b| {
                    let cmp = match (a, b) {
                        (OwnedValue::Integer(x), OwnedValue::Integer(y)) => x.cmp(y),
                        (OwnedValue::Decimal(x), OwnedValue::Decimal(y)) =>
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                        (OwnedValue::Str(x), OwnedValue::Str(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if descending { cmp.reverse() } else { cmp }
                });

                let new_refs: Vec<ObjectRef> = owned_vals.into_iter().map(|v| {
                    match arr_ref.region {
                        RegionId::Global => self.plant_global(v),
                        RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(v),
                        RegionId::Scoped => self.plant(v),
                    }
                }).collect();
                self.update_array(arr_ref, element_type, new_refs);
                EvalResult::Value(arr_ref)
            }

            "map" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: map expects 1 callback argument");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let n_params = match self.callback_param_count(cb_ref) {
                    Some(n) => n,
                    None => { eprintln!("❌ ERROR: map argument must be a function"); return EvalResult::Error; }
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut results = Vec::new();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let args = if n_params >= 2 {
                        vec![val, OwnedValue::Integer(i as i64)]
                    } else {
                        vec![val]
                    };
                    match self.call_function(cb_ref, args) {
                        EvalResult::Value(r) => results.push(r),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: results }))
            }

            "filter" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: filter expects 1 callback argument");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let n_params = match self.callback_param_count(cb_ref) {
                    Some(n) => n,
                    None => { eprintln!("❌ ERROR: filter argument must be a function"); return EvalResult::Error; }
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut kept = Vec::new();
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
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if keep {
                        kept.push(self.plant(val));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type, elements: kept }))
            }

            "reduce" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    eprintln!("❌ ERROR: reduce expects 1 argument (callback) or 2 (initial, callback)");
                    return EvalResult::Error;
                }
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let (mut acc_ref, cb_ref, start_idx) = if dot_call.arguments.len() == 2 {
                    // reduce(initial, callback)
                    let init = match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    let cb = match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    (init, cb, 0usize)
                } else {
                    // reduce(callback) — first element is the initial accumulator
                    if owned_elems.is_empty() {
                        eprintln!("❌ ERROR: reduce with no initial value requires a non-empty array");
                        return EvalResult::Error;
                    }
                    let cb = match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    let first_ref = self.plant(owned_elems[0].clone());
                    (first_ref, cb, 1usize)
                };
                for val in owned_elems.into_iter().skip(start_idx) {
                    let acc_val = self.extract(acc_ref);
                    acc_ref = match self.call_function(cb_ref, vec![acc_val, val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                }
                EvalResult::Value(acc_ref)
            }

            "join" => {
                let sep = if dot_call.arguments.is_empty() {
                    ",".to_string()
                } else {
                    match self.eval_str_arg(&dot_call.arguments[0]) {
                        Some(s) => s,
                        None => return EvalResult::Error,
                    }
                };
                let parts: Vec<String> = elems.iter()
                    .map(|&r| self.display(r))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Str(parts.join(&sep))))
            }

            "toString" => {
                let s = self.display(arr_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            "indexOf" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: indexOf expects 1 argument");
                    return EvalResult::Error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let idx = elems.iter().enumerate().find(|(_, elem)| {
                    let elem_data = self.resolve(**elem).cloned();
                    obj_data_eq(&elem_data, &needle_data)
                }).map(|(i, _)| i as i64).unwrap_or(-1);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx)))
            }

            "includes" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: includes expects 1 argument");
                    return EvalResult::Error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let found = elems.iter().any(|elem| {
                    let elem_data = self.resolve(*elem).cloned();
                    obj_data_eq(&elem_data, &needle_data)
                });
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }

            "find" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: find expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let val_clone = val.clone();
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.plant(val_clone));
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            "findIndex" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: findIndex expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Integer(i as i64)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(-1)))
            }

            "slice" => {
                let len = elems.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => 0 },
                        _ => return EvalResult::Error,
                    }
                } else { 0 };
                let end_i = if dot_call.arguments.len() >= 2 {
                    match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => len },
                        _ => return EvalResult::Error,
                    }
                } else { len };
                // Normalize negative indices (count from end) then clamp
                let start = (if start_i < 0 { (len + start_i).max(0) } else { start_i.min(len) }) as usize;
                let end   = (if end_i   < 0 { (len + end_i  ).max(0) } else { end_i.min(len)   }) as usize;
                let end = end.max(start); // prevent inverted range
                let sliced: Vec<ObjectRef> = elems[start..end].iter().map(|r| {
                    let owned = self.extract(*r);
                    self.plant(owned)
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: element_type.clone(), elements: sliced }))
            }

            "reverse" => {
                let mut e = elems;
                e.reverse();
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(arr_ref)
            }

            "every" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: every expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if !self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(false)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(true)))
            }

            "some" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: some expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(self.resolve(result).unwrap()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(true)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(false)))
            }

            "flat" => {
                let depth = if dot_call.arguments.is_empty() {
                    1usize
                } else {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) {
                            Some(ObjectData::Integer(d)) => (*d).max(0) as usize,
                            _ => 1,
                        },
                        _ => return EvalResult::Error,
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

                let owned_elems: Vec<OwnedValue> = elems.iter().map(|r| self.extract(*r)).collect();
                let flat = flat_owned(owned_elems, depth);
                let refs: Vec<ObjectRef> = flat.into_iter().map(|v| self.plant(v)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown array method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── String methods ────────────────────────────────────────────────────────

}
