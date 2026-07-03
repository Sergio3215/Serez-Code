#![allow(unused_imports)]
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use super::{EvalResult, StoredClass, CallFrame, type_matches, obj_data_to_key_str,
            obj_data_eq, format_decimal, json_stringify_owned, json_parse,
            operator_to_method_name, owned_to_obj_data};
use crate::region::{set_key_str, SetIndex, SET_INDEX_MIN};

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
                _ => return EvalResult::Error,
            };
            if let Some(ObjectData::Array { elements: arr_elems, .. }) = self.resolve(arr_ref).cloned() {
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
        EvalResult::Value(self.alloc(ObjectData::Set { elements, index: Default::default() }))
    }

    /// True when the set contains `vd`, honouring obj_data_eq semantics.
    /// Scalar probe on a big set → O(1) via the slot-resident index; small set
    /// or fingerprint-less probe → linear scan (same behavior as always).
    fn set_contains(elements: &[OwnedValue], index: &SetIndex, vd: &OwnedValue, key: Option<&str>) -> bool {
        match key {
            Some(k) if elements.len() >= SET_INDEX_MIN => index.lookup(elements, k).is_some(),
            Some(k) => elements.iter().any(|e| set_key_str(e).as_deref() == Some(k)),
            // No fingerprint = compound value: obj_data_eq never matches those,
            // but keep the authoritative scan in case its semantics ever widen.
            None => elements.iter().any(|e| {
                obj_data_eq(&Some(owned_to_obj_data(e)), &Some(owned_to_obj_data(vd)))
            }),
        }
    }

    /// Fast path for `has`/`contains`/`add` dispatched from expr.rs BEFORE the
    /// generic dot-call clones the receiver: runs against the arena slot, so a
    /// 20k-element set is not copied per call and its index stays warm.
    pub(super) fn eval_set_fast(&mut self, set_ref: ObjectRef, dot_call: &ast::DotCallExpression) -> EvalResult {
        let vr = match self.eval_expression(&dot_call.arguments[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let vd = self.extract(vr);
        let key = set_key_str(&vd);
        match dot_call.method.as_str() {
            "has" | "contains" => {
                let found = match self.resolve(set_ref) {
                    Some(ObjectData::Set { elements, index }) => {
                        Self::set_contains(elements, index, &vd, key.as_deref())
                    }
                    _ => false,
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }
            "add" => {
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
            _ => EvalResult::Error, // unreachable: dispatcher only routes has/contains/add
        }
    }

    pub(super) fn eval_set_method(&mut self, set_ref: ObjectRef, elements: Vec<OwnedValue>, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "size" => EvalResult::Value(self.alloc(ObjectData::Integer(elements.len() as i64))),
            "toArray" => {
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements }))
            }
            "has" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.has(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.extract(vr);
                let found = elements.iter().any(|elem| obj_data_eq(&Some(owned_to_obj_data(elem)), &Some(owned_to_obj_data(&vd))));
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }
            "add" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.add(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.extract(vr);
                let already = elements.iter().any(|elem| obj_data_eq(&Some(owned_to_obj_data(elem)), &Some(owned_to_obj_data(&vd))));
                if !already {
                    let mut new_elems = elements;
                    new_elems.push(vd);
                    self.update_set(set_ref, new_elems);
                }
                EvalResult::Value(set_ref)
            }
            "delete" | "remove" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.delete(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.extract(vr);
                let before = elements.len();
                let new_elems: Vec<OwnedValue> = elements.into_iter().filter(|elem| !obj_data_eq(&Some(owned_to_obj_data(elem)), &Some(owned_to_obj_data(&vd)))).collect();
                let removed = new_elems.len() < before;
                self.update_set(set_ref, new_elems);
                EvalResult::Value(self.alloc(ObjectData::Boolean(removed)))
            }
            "clear" => {
                self.update_set(set_ref, vec![]);
                EvalResult::Value(self.null_ref)
            }
            "union" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.union(other) requires 1 argument"); return EvalResult::Error;
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let other_elems = match self.resolve(or).cloned() {
                    Some(ObjectData::Set { elements: oe, .. }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.union requires a Set argument"); return EvalResult::Error; }
                };
                let mut result: Vec<OwnedValue> = elements.clone();
                for elem in other_elems {
                    if !result.iter().any(|e| obj_data_eq(&Some(owned_to_obj_data(e)), &Some(owned_to_obj_data(&elem)))) {
                        result.push(elem);
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result, index: Default::default() }))
            }
            "intersection" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.intersection(other) requires 1 argument"); return EvalResult::Error;
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let other_elems = match self.resolve(or).cloned() {
                    Some(ObjectData::Set { elements: oe, .. }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.intersection requires a Set argument"); return EvalResult::Error; }
                };
                let result: Vec<OwnedValue> = elements.into_iter().filter(|elem| {
                    other_elems.iter().any(|other| obj_data_eq(&Some(owned_to_obj_data(elem)), &Some(owned_to_obj_data(other))))
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result, index: Default::default() }))
            }
            "toString" => {
                let s = self.display(set_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }
            _ => {
                eprintln!("❌ ERROR: Unknown Set method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    pub(super) fn update_set(&mut self, set_ref: ObjectRef, new_elements: Vec<OwnedValue>) {
        let new_data = ObjectData::Set { elements: new_elements, index: Default::default() };
        match set_ref.region {
            RegionId::Global => self.global_arena.update(set_ref.index, new_data),
            RegionId::Scoped => self.scopes.arena.update(set_ref.index, new_data),
        }
    }
}
