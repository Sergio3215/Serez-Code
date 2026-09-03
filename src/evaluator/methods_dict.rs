use super::ExecutionFlow;
use super::{EvalResult, obj_data_to_key_str, owned_to_key_str, type_matches};
use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};

impl super::Evaluator {
    /// Position of `probe` under the LEGACY comparator (`owned_to_key_str`),
    /// resolved through the slot-resident hash index instead of a linear scan.
    ///
    /// The index canonicalizes keys with `dict_key_str`, which is not the same
    /// function: it also renders `Decimal` and compound keys, where the legacy
    /// comparator yields `""`. That difference is contained here:
    ///
    /// * `probe` is never empty when this runs (callers check), so an index MISS
    ///   is a true miss — an entry the legacy scan would have matched must be a
    ///   Str/Integer/Boolean key with exactly that string, and for those two
    ///   canonicalizations agree, so the index would have found it. This is what
    ///   makes building a dict key-by-key O(1) per insert: the miss costs nothing.
    /// * an index HIT is validated with the legacy comparator, and a hit the
    ///   legacy scan would reject (a `Decimal` key aliasing onto the probe) falls
    ///   back to the scan. Rare enough to be free, and semantics stay identical.
    fn dict_pos(
        entries: &[(OwnedValue, OwnedValue)],
        index: &crate::region::DictIndex,
        probe: &str,
    ) -> Option<usize> {
        match index.lookup(entries, probe) {
            Some(i) if owned_to_key_str(&entries[i].0) == probe => Some(i),
            Some(_) => entries
                .iter()
                .position(|(k, _)| owned_to_key_str(k) == probe),
            None => None,
        }
    }

    /// All dict methods, dispatched from expr.rs BEFORE the generic dot-call
    /// clones the receiver.
    ///
    /// The generic path deep-cloned every entry to service a call, mutated the
    /// copy and rewrote the whole slot through `update_dict` — which also reset
    /// the hash index, so the next read had to rebuild it. Building a dict with
    /// `d.Add(...)` in a loop was therefore O(N²) twice over (clone per call,
    /// plus a linear duplicate-key scan per call), while the very same insert
    /// written `d[k] = v` had been O(1) since 7.3.0.
    ///
    /// Every method here runs against the arena slot: reads clone only what they
    /// return, mutations happen in place, and the index stays warm.
    /// Reject a dict call that takes no arguments but was given some. Checked
    /// before anything is evaluated, so a rejected call runs no side effects.
    fn dict_no_args(&mut self, dot_call: &ast::DotCallExpression) -> Option<EvalResult> {
        if dot_call.arguments.is_empty() {
            return None;
        }
        let method = dot_call.method.as_str();
        let given = dot_call.arguments.len();
        Some(self.rt_err_kind(
            "TypeError",
            format!("{method} expects 0 arguments, got {given}"),
        ))
    }

    /// The receiver was dispatched here as a dict; if the slot says otherwise an
    /// internal invariant broke. Reported rather than silently answered with an
    /// empty result, which is how a dispatcher bug used to look like empty data.
    fn dict_receiver_broken(&mut self, method: &str) -> EvalResult {
        self.rt_err_kind("TypeError", format!("{method}: receiver is not a dict"))
    }

    pub(super) fn eval_dict_method_slot(
        &mut self,
        dict_ref: ObjectRef,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "Add" => self.dict_add(dict_ref, dot_call),
            "Remove" => self.dict_remove(dict_ref, dot_call),

            "RemoveAll" | "clear" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let arena = match dict_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                if let Some(ObjectData::Dict { entries, .. }) = arena.get_mut(dict_ref.index) {
                    entries.clear();
                }
                Ok(ExecutionFlow::Value(self.null_ref))
            }

            // Returns array of keys: [k1, k2, ...]
            "toList" | "keys" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let keys: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => {
                        entries.iter().map(|(k, _)| k.clone()).collect()
                    }
                    _ => return self.dict_receiver_broken("keys"),
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: keys,
                })))
            }

            "values" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let vals: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => {
                        entries.iter().map(|(_, v)| v.clone()).collect()
                    }
                    _ => return self.dict_receiver_broken("values"),
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: vals,
                })))
            }

            // Returns 2-D array of entries: [[k1,v1],[k2,v2],...]
            "toArray" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let pairs: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => entries
                        .iter()
                        .map(|(k, v)| OwnedValue::Array {
                            element_type: None,
                            elements: vec![k.clone(), v.clone()],
                        })
                        .collect(),
                    _ => return self.dict_receiver_broken("toArray"),
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: pairs,
                })))
            }

            // Reached only through a dict living somewhere the upstream length()
            // fast path does not cover; O(1) either way.
            "length" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let n = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => entries.len() as i64,
                    _ => return self.dict_receiver_broken("length"),
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(n))))
            }

            "toString" => {
                if let Some(error) = self.dict_no_args(dot_call) {
                    return error;
                }
                let s = self.display(dict_ref);
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(s))))
            }

            unknown => {
                let message = format!("Unknown dict method '{unknown}'");
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    /// Check a value against a dict's declared key/value type.
    /// Returns the offending type name when the value does not belong.
    fn dict_type_mismatch(&self, value: ObjectRef, declared: &str) -> Option<String> {
        match self.resolve(value) {
            Some(data) if type_matches(declared, data) => None,
            Some(data) => Some(data.type_name().to_string()),
            None => Some("null".to_string()),
        }
    }

    /// `d.Add({key, value})` — replaces the value when the key is present,
    /// appends otherwise. Same insert `d[key] = value` performs, and now at the
    /// same cost: one indexed probe, then an in-place write or a push.
    fn dict_add(&mut self, dict_ref: ObjectRef, dot_call: &ast::DotCallExpression) -> EvalResult {
        if dot_call.arguments.len() != 1 {
            let given = dot_call.arguments.len();
            return self.rt_err_kind(
                "TypeError",
                format!("Add expects 1 argument {{key, value}}, got {given}"),
            );
        }
        let (key_ref, val_ref) = match &dot_call.arguments[0] {
            ast::Expression::EntryLiteral {
                key: k_expr,
                value: v_expr,
                ..
            } => {
                let k = match self.eval_expression(k_expr) {
                    Ok(ExecutionFlow::Value(r)) => r,
                    other => return other,
                };
                let v = match self.eval_expression(v_expr) {
                    Ok(ExecutionFlow::Value(r)) => r,
                    other => return other,
                };
                (k, v)
            }
            _ => {
                return self.rt_err_kind(
                    "TypeError",
                    "Add: argument must be an entry literal {key, value}",
                );
            }
        };

        // The declared types are read AFTER the arguments are evaluated — they
        // are part of the receiver's slot, and only the two Strings are copied.
        let (key_type, value_type) = match self.resolve(dict_ref) {
            Some(ObjectData::Dict {
                key_type,
                value_type,
                ..
            }) => (key_type.clone(), value_type.clone()),
            _ => return self.dict_receiver_broken("Add"),
        };

        if key_type != "any" {
            if let Some(actual) = self.dict_type_mismatch(key_ref, &key_type) {
                return self.rt_err_kind(
                    "TypeError",
                    format!("Dict key type mismatch on Add: expected '{key_type}', got '{actual}'"),
                );
            }
        }
        if value_type != "any" {
            if let Some(actual) = self.dict_type_mismatch(val_ref, &value_type) {
                return self.rt_err_kind(
                    "TypeError",
                    format!(
                        "Dict value type mismatch on Add: expected '{value_type}', got '{actual}'"
                    ),
                );
            }
        }

        let search_key = match self.resolve(key_ref) {
            Some(data) => obj_data_to_key_str(data),
            None => return self.dict_receiver_broken("Add"),
        };
        let owned_k = self.extract(key_ref);
        let owned_v = self.extract(val_ref);

        let arena = match dict_ref.region {
            RegionId::Global => &mut self.global_arena,
            RegionId::Scoped => &mut self.scopes.arena,
        };
        if let Some(ObjectData::Dict { entries, index, .. }) = arena.get_mut(dict_ref.index) {
            // An empty canonical key (a Decimal or compound key, or the literal
            // "") is outside what the index can answer for: keep the historical
            // linear scan so those keys behave exactly as they always did.
            let pos = if search_key.is_empty() {
                entries
                    .iter()
                    .position(|(k, _)| owned_to_key_str(k).is_empty())
            } else {
                Self::dict_pos(entries, index, &search_key)
            };
            match pos {
                Some(i) => entries[i].1 = owned_v,
                None => {
                    entries.push((owned_k, owned_v));
                    if !search_key.is_empty() {
                        index.record_append(&search_key, entries.len() - 1);
                    }
                }
            }
        }
        Ok(ExecutionFlow::Value(self.null_ref))
    }

    /// `d.Remove(key)` — drops every entry under that key (a dict literal is the
    /// one way to end up with duplicates, and the historical `retain` dropped
    /// them all). The indexed probe makes the common "key is not there" case
    /// O(1); an actual removal still shifts the tail of the Vec.
    fn dict_remove(
        &mut self,
        dict_ref: ObjectRef,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        if dot_call.arguments.len() != 1 {
            let given = dot_call.arguments.len();
            return self.rt_err_kind(
                "TypeError",
                format!("Remove expects 1 argument (key), got {given}"),
            );
        }
        let key_ref = match self.eval_expression(&dot_call.arguments[0]) {
            Ok(ExecutionFlow::Value(r)) => r,
            other => return other,
        };
        let search_key = match self.resolve(key_ref) {
            Some(data) => obj_data_to_key_str(data),
            None => return self.dict_receiver_broken("Remove"),
        };

        let arena = match dict_ref.region {
            RegionId::Global => &mut self.global_arena,
            RegionId::Scoped => &mut self.scopes.arena,
        };
        if let Some(ObjectData::Dict { entries, index, .. }) = arena.get_mut(dict_ref.index) {
            let present = if search_key.is_empty() {
                entries.iter().any(|(k, _)| owned_to_key_str(k).is_empty())
            } else {
                Self::dict_pos(entries, index, &search_key).is_some()
            };
            if present {
                entries.retain(|(k, _)| owned_to_key_str(k) != search_key);
            }
        }
        Ok(ExecutionFlow::Value(self.null_ref))
    }
}
