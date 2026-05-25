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
    pub(super) fn eval_string_method(&mut self, s: String, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {

            "length" => EvalResult::Value(self.alloc(ObjectData::Integer(s.chars().count() as i64))),

            "toString" => EvalResult::Value(self.alloc(ObjectData::Str(s))),

            "trim" => EvalResult::Value(self.alloc(ObjectData::Str(s.trim().to_string()))),

            "toUpperCase" | "upper" => EvalResult::Value(self.alloc(ObjectData::Str(s.to_uppercase()))),

            "toLowerCase" | "lower" => EvalResult::Value(self.alloc(ObjectData::Str(s.to_lowercase()))),

            "startsWith" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: startsWith expects 1 argument");
                    return EvalResult::Error;
                }
                let prefix = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.starts_with(&prefix[..]))))
            }

            "endsWith" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: endsWith expects 1 argument");
                    return EvalResult::Error;
                }
                let suffix = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.ends_with(&suffix[..]))))
            }

            "indexOf" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: indexOf expects 1 argument (substring)");
                    return EvalResult::Error;
                }
                let needle = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let idx: i64 = if needle.is_empty() {
                    0
                } else {
                    // Search in character space, not byte space
                    let haystack: Vec<char> = s.chars().collect();
                    let needle_chars: Vec<char> = needle.chars().collect();
                    let mut found = -1i64;
                    'search: for i in 0..haystack.len() {
                        if haystack.len() - i < needle_chars.len() { break; }
                        for j in 0..needle_chars.len() {
                            if haystack[i + j] != needle_chars[j] { continue 'search; }
                        }
                        found = i as i64;
                        break;
                    }
                    found
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(idx)))
            }

            "charAt" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: charAt expects 1 argument (index)");
                    return EvalResult::Error;
                }
                let idx = match self.eval_int_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let chars: Vec<char> = s.chars().collect();
                let result = if idx < 0 || idx as usize >= chars.len() {
                    String::new()
                } else {
                    chars[idx as usize].to_string()
                };
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "includes" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: includes expects 1 argument");
                    return EvalResult::Error;
                }
                let sub = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(t)) => t,
                        _ => { eprintln!("❌ ERROR: includes argument must be a string"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.contains(&sub[..]))))
            }

            "replace" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: replace expects 2 arguments (from, to)");
                    return EvalResult::Error;
                }
                let from = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let to   = match self.eval_str_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error };
                if from.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                EvalResult::Value(self.alloc(ObjectData::Str(s.replacen(&from[..], &to, 1))))
            }

            "replaceAll" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: replaceAll expects 2 arguments (from, to)");
                    return EvalResult::Error;
                }
                let from = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let to   = match self.eval_str_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error };
                if from.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                EvalResult::Value(self.alloc(ObjectData::Str(s.replace(&from[..], &to))))
            }

            "split" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: split expects 1 argument (separator)");
                    return EvalResult::Error;
                }
                let sep = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let parts: Vec<ObjectRef> = if sep.is_empty() {
                    // Empty separator → split into individual characters
                    s.chars().map(|c| self.alloc(ObjectData::Str(c.to_string()))).collect()
                } else {
                    s.split(&sep[..]).map(|p| self.alloc(ObjectData::Str(p.to_string()))).collect()
                };
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: parts }))
            }

            "substring" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    eprintln!("❌ ERROR: substring expects 1 or 2 arguments (start [, end])");
                    return EvalResult::Error;
                }
                let start = match self.eval_int_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let end = if dot_call.arguments.len() == 2 {
                    match self.eval_int_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error }
                } else {
                    len
                };
                let start = start.max(0).min(len) as usize;
                let end   = end.max(0).min(len) as usize;
                let start = start.min(end);
                let result: String = chars[start..end].iter().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "padStart" => {
                if dot_call.arguments.len() < 1 {
                    eprintln!("❌ ERROR: padStart expects at least 1 argument");
                    return EvalResult::Error;
                }
                let target_len = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i as usize, _ => 0 },
                    _ => return EvalResult::Error,
                };
                let pad_str = if dot_call.arguments.len() >= 2 {
                    self.eval_str_arg(&dot_call.arguments[1]).unwrap_or_else(|| " ".to_string())
                } else { " ".to_string() };
                let s_len = s.chars().count();
                if s_len >= target_len || pad_str.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                let mut result = s.clone();
                while result.chars().count() < target_len {
                    result = pad_str.clone() + &result;
                }
                // Trim to exact length if pad_str is multi-char and overshot
                let result: String = result.chars().rev().take(target_len).collect::<Vec<_>>().into_iter().rev().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "padEnd" => {
                if dot_call.arguments.len() < 1 {
                    eprintln!("❌ ERROR: padEnd expects at least 1 argument");
                    return EvalResult::Error;
                }
                let target_len = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i as usize, _ => 0 },
                    _ => return EvalResult::Error,
                };
                let pad_str = if dot_call.arguments.len() >= 2 {
                    self.eval_str_arg(&dot_call.arguments[1]).unwrap_or_else(|| " ".to_string())
                } else { " ".to_string() };
                if s.chars().count() >= target_len || pad_str.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                let mut result = s.clone();
                while result.chars().count() < target_len {
                    result.push_str(&pad_str);
                }
                let result: String = result.chars().take(target_len).collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "slice" => {
                let chars: Vec<char> = s.chars().collect();
                let slen = chars.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => 0 },
                        _ => return EvalResult::Error,
                    }
                } else { 0 };
                let end_i = if dot_call.arguments.len() >= 2 {
                    match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => slen },
                        _ => return EvalResult::Error,
                    }
                } else { slen };
                let start = (if start_i < 0 { (slen + start_i).max(0) } else { start_i.min(slen) }) as usize;
                let end   = (if end_i   < 0 { (slen + end_i  ).max(0) } else { end_i.min(slen)   }) as usize;
                let end = end.max(start);
                let sliced: String = chars[start..end].iter().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(sliced)))
            }

            "trimStart" | "trimLeft" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.trim_start().to_string())))
            }

            "trimEnd" | "trimRight" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.trim_end().to_string())))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown string method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Argument extraction helpers ───────────────────────────────────────────

}
