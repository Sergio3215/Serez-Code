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
    pub(super) fn eval_new_set(&mut self, new_expr: &ast::NewExpression) -> EvalResult {
        let mut elements: Vec<ObjectRef> = Vec::new();
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
                for elem_ref in arr_elems {
                    let elem_data = self.resolve(elem_ref).cloned();
                    let already = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &elem_data));
                    if !already {
                        elements.push(elem_ref);
                    }
                }
            }
        }
        EvalResult::Value(self.alloc(ObjectData::Set { elements }))
    }

    pub(super) fn eval_set_method(&mut self, set_ref: ObjectRef, elements: Vec<ObjectRef>, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "size" => EvalResult::Value(self.alloc(ObjectData::Integer(elements.len() as i64))),
            "toArray" => {
                let owned: Vec<OwnedValue> = elements.iter().map(|&r| self.extract(r)).collect();
                let refs: Vec<ObjectRef> = owned.into_iter().map(|v| self.plant(v)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
            }
            "has" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.has(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let found = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &vd));
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }
            "add" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.add(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let already = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &vd));
                if !already {
                    let owned = self.extract(vr);
                    let new_ref = self.plant_global(owned);
                    let mut new_elems = elements;
                    new_elems.push(new_ref);
                    self.update_set(set_ref, new_elems);
                }
                EvalResult::Value(set_ref)
            }
            "delete" | "remove" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.delete(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let before = elements.len();
                let new_elems: Vec<ObjectRef> = elements.into_iter().filter(|&er| !obj_data_eq(&self.resolve(er).cloned(), &vd)).collect();
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
                    Some(ObjectData::Set { elements: oe }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.union requires a Set argument"); return EvalResult::Error; }
                };
                let mut result: Vec<ObjectRef> = elements.clone();
                for er in other_elems {
                    let ed = self.resolve(er).cloned();
                    if !result.iter().any(|&rr| obj_data_eq(&self.resolve(rr).cloned(), &ed)) {
                        let owned = self.extract(er);
                        result.push(self.plant_global(owned));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result }))
            }
            "intersection" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.intersection(other) requires 1 argument"); return EvalResult::Error;
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let other_elems = match self.resolve(or).cloned() {
                    Some(ObjectData::Set { elements: oe }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.intersection requires a Set argument"); return EvalResult::Error; }
                };
                let result: Vec<ObjectRef> = elements.into_iter().filter(|&er| {
                    let ed = self.resolve(er).cloned();
                    other_elems.iter().any(|&oer| obj_data_eq(&self.resolve(oer).cloned(), &ed))
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result }))
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

    pub(super) fn update_set(&mut self, set_ref: ObjectRef, new_elements: Vec<ObjectRef>) {
        let new_data = ObjectData::Set { elements: new_elements };
        match set_ref.region {
            RegionId::Global => self.global_arena.update(set_ref.index, new_data),
            RegionId::Scoped => self.scopes.arena.update(set_ref.index, new_data),
        }
    }
}
