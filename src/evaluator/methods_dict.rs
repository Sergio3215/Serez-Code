use crate::ast;
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use super::{EvalResult, type_matches, obj_data_to_key_str, owned_to_key_str};

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
            Some(_) => entries.iter().position(|(k, _)| owned_to_key_str(k) == probe),
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
    pub(super) fn eval_dict_method_slot(
        &mut self,
        dict_ref: ObjectRef,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "Add" => self.dict_add(dict_ref, dot_call),
            "Remove" => self.dict_remove(dict_ref, dot_call),

            "RemoveAll" | "clear" => {
                if !dot_call.arguments.is_empty() {
                    eprintln!("❌ ERROR: {} expects no arguments", dot_call.method);
                    return EvalResult::Error;
                }
                let arena = match dict_ref.region {
                    RegionId::Global => &mut self.global_arena,
                    RegionId::Scoped => &mut self.scopes.arena,
                };
                if let Some(ObjectData::Dict { entries, .. }) = arena.get_mut(dict_ref.index) {
                    entries.clear();
                }
                EvalResult::Value(self.null_ref)
            }

            // Returns array of keys: [k1, k2, ...]
            "toList" | "keys" => {
                let keys: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => {
                        entries.iter().map(|(k, _)| k.clone()).collect()
                    }
                    _ => Vec::new(),
                };
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: keys }))
            }

            "values" => {
                let vals: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => {
                        entries.iter().map(|(_, v)| v.clone()).collect()
                    }
                    _ => Vec::new(),
                };
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: vals }))
            }

            // Returns 2-D array of entries: [[k1,v1],[k2,v2],...]
            "toArray" => {
                let pairs: Vec<OwnedValue> = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => entries
                        .iter()
                        .map(|(k, v)| OwnedValue::Array {
                            element_type: None,
                            elements: vec![k.clone(), v.clone()],
                        })
                        .collect(),
                    _ => Vec::new(),
                };
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: pairs }))
            }

            // Reached only through a dict living somewhere the upstream length()
            // fast path does not cover; O(1) either way.
            "length" => {
                let n = match self.resolve(dict_ref) {
                    Some(ObjectData::Dict { entries, .. }) => entries.len() as i64,
                    _ => 0,
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(n)))
            }

            "toString" => {
                let s = self.display(dict_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown dict method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    /// `d.Add({key, value})` — replaces the value when the key is present,
    /// appends otherwise. Same insert `d[key] = value` performs, and now at the
    /// same cost: one indexed probe, then an in-place write or a push.
    fn dict_add(&mut self, dict_ref: ObjectRef, dot_call: &ast::DotCallExpression) -> EvalResult {
        if dot_call.arguments.len() != 1 {
            eprintln!("❌ ERROR: Add expects 1 argument {{key, value}}");
            return EvalResult::Error;
        }
        let (key_ref, val_ref) = match &dot_call.arguments[0] {
            ast::Expression::EntryLiteral(k_expr, v_expr) => {
                let k = match self.eval_expression(k_expr) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let v = match self.eval_expression(v_expr) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                (k, v)
            }
            _ => {
                eprintln!("❌ ERROR: Add argument must be an entry literal {{key, value}}");
                return EvalResult::Error;
            }
        };

        // The declared types are read AFTER the arguments are evaluated — they
        // are part of the receiver's slot, and only the two Strings are copied.
        let (key_type, value_type) = match self.resolve(dict_ref) {
            Some(ObjectData::Dict { key_type, value_type, .. }) => {
                (key_type.clone(), value_type.clone())
            }
            _ => return EvalResult::Error,
        };

        if key_type != "any" {
            let kd = self.resolve(key_ref).unwrap();
            if !type_matches(&key_type, kd) {
                eprintln!("❌ TYPE ERROR: Dict key type mismatch on Add (expected '{}')", key_type);
                return EvalResult::Error;
            }
        }
        if value_type != "any" {
            let vd = self.resolve(val_ref).unwrap();
            if !type_matches(&value_type, vd) {
                eprintln!("❌ TYPE ERROR: Dict value type mismatch on Add (expected '{}')", value_type);
                return EvalResult::Error;
            }
        }

        let search_key = obj_data_to_key_str(self.resolve(key_ref).unwrap());
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
                entries.iter().position(|(k, _)| owned_to_key_str(k).is_empty())
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
        EvalResult::Value(self.null_ref)
    }

    /// `d.Remove(key)` — drops every entry under that key (a dict literal is the
    /// one way to end up with duplicates, and the historical `retain` dropped
    /// them all). The indexed probe makes the common "key is not there" case
    /// O(1); an actual removal still shifts the tail of the Vec.
    fn dict_remove(&mut self, dict_ref: ObjectRef, dot_call: &ast::DotCallExpression) -> EvalResult {
        if dot_call.arguments.len() != 1 {
            eprintln!("❌ ERROR: Remove expects 1 argument (key)");
            return EvalResult::Error;
        }
        let key_ref = match self.eval_expression(&dot_call.arguments[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let search_key = obj_data_to_key_str(self.resolve(key_ref).unwrap());

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
        EvalResult::Value(self.null_ref)
    }
}
