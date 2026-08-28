#![allow(unused_imports)]
use super::{
    CallFrame, EvalResult, StoredClass, format_decimal, json_parse, json_stringify_owned,
    obj_data_eq, obj_data_to_key_str, operator_to_method_name, type_matches,
};
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

const MAX_PADDED_STRING_CHARS: usize = 10_000_000;

impl super::Evaluator {
    pub(super) fn eval_string_method(
        &mut self,
        s: String,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {
            "length" | "toString" | "trim" | "toUpperCase" | "upper" | "toLowerCase" | "lower"
            | "trimStart" | "trimLeft" | "trimEnd" | "trimRight"
                if !dot_call.arguments.is_empty() =>
            {
                self.rt_err_kind(
                    "TypeError",
                    format!("String.{}() requires 0 arguments", dot_call.method),
                )
            }
            "length" => {
                EvalResult::Value(self.alloc(ObjectData::Integer(s.chars().count() as i64)))
            }

            "toString" => EvalResult::Value(self.alloc(ObjectData::Str(s))),

            "trim" => EvalResult::Value(self.alloc(ObjectData::Str(s.trim().to_string()))),

            "toUpperCase" | "upper" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.to_uppercase())))
            }

            "toLowerCase" | "lower" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.to_lowercase())))
            }

            "startsWith" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "String.startsWith expects 1 argument");
                }
                let prefix =
                    match self.eval_string_arg(&dot_call.arguments[0], "startsWith", "prefix") {
                        Ok(value) => value,
                        Err(error) => return error,
                    };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.starts_with(&prefix[..]))))
            }

            "endsWith" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "String.endsWith expects 1 argument");
                }
                let suffix =
                    match self.eval_string_arg(&dot_call.arguments[0], "endsWith", "suffix") {
                        Ok(value) => value,
                        Err(error) => return error,
                    };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.ends_with(&suffix[..]))))
            }

            "indexOf" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "String.indexOf expects 1 argument (substring)");
                }
                let needle =
                    match self.eval_string_arg(&dot_call.arguments[0], "indexOf", "substring") {
                        Ok(value) => value,
                        Err(error) => return error,
                    };
                let idx: i64 = if needle.is_empty() {
                    0
                } else {
                    // Search in character space, not byte space
                    let haystack: Vec<char> = s.chars().collect();
                    let needle_chars: Vec<char> = needle.chars().collect();
                    let mut found = -1i64;
                    'search: for i in 0..haystack.len() {
                        if haystack.len() - i < needle_chars.len() {
                            break;
                        }
                        for j in 0..needle_chars.len() {
                            if haystack[i + j] != needle_chars[j] {
                                continue 'search;
                            }
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
                    return self
                        .rt_err_kind("TypeError", "String.charAt expects 1 argument (index)");
                }
                let idx = match self.eval_string_int_arg(&dot_call.arguments[0], "charAt", "index")
                {
                    Ok(value) => value,
                    Err(error) => return error,
                };
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
                    return self.rt_err_kind(
                        "TypeError",
                        format!("String.{} expects 1 argument", dot_call.method),
                    );
                }
                let sub = match self.eval_string_arg(
                    &dot_call.arguments[0],
                    &dot_call.method,
                    "substring",
                ) {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.contains(&sub[..]))))
            }

            "replace" => {
                if dot_call.arguments.len() != 2 {
                    return self
                        .rt_err_kind("TypeError", "String.replace expects 2 arguments (from, to)");
                }
                let from = match self.eval_string_arg(&dot_call.arguments[0], "replace", "from") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let to = match self.eval_string_arg(&dot_call.arguments[1], "replace", "to") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                if from.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                EvalResult::Value(self.alloc(ObjectData::Str(s.replacen(&from[..], &to, 1))))
            }

            "replaceAll" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind(
                        "TypeError",
                        "String.replaceAll expects 2 arguments (from, to)",
                    );
                }
                let from = match self.eval_string_arg(&dot_call.arguments[0], "replaceAll", "from")
                {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let to = match self.eval_string_arg(&dot_call.arguments[1], "replaceAll", "to") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                if from.is_empty() {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                EvalResult::Value(self.alloc(ObjectData::Str(s.replace(&from[..], &to))))
            }

            "split" => {
                if dot_call.arguments.len() != 1 {
                    return self
                        .rt_err_kind("TypeError", "String.split expects 1 argument (separator)");
                }
                let sep = match self.eval_string_arg(&dot_call.arguments[0], "split", "separator") {
                    Ok(value) => value,
                    Err(error) => return error,
                };
                let parts: Vec<OwnedValue> = if sep.is_empty() {
                    // Empty separator → split into individual characters
                    s.chars().map(|c| OwnedValue::Str(c.to_string())).collect()
                } else {
                    s.split(&sep[..])
                        .map(|p| OwnedValue::Str(p.to_string()))
                        .collect()
                };
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: None,
                    elements: parts,
                }))
            }

            "substring" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    return self.rt_err_kind(
                        "TypeError",
                        "String.substring expects 1 or 2 arguments (start [, end])",
                    );
                }
                let start =
                    match self.eval_string_int_arg(&dot_call.arguments[0], "substring", "start") {
                        Ok(value) => value,
                        Err(error) => return error,
                    };
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let end = if dot_call.arguments.len() == 2 {
                    match self.eval_string_int_arg(&dot_call.arguments[1], "substring", "end") {
                        Ok(value) => value,
                        Err(error) => return error,
                    }
                } else {
                    len
                };
                let start = start.max(0).min(len) as usize;
                let end = end.max(0).min(len) as usize;
                let start = start.min(end);
                let result: String = chars[start..end].iter().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "padStart" => {
                if !(1..=2).contains(&dot_call.arguments.len()) {
                    return self.rt_err_kind(
                        "TypeError",
                        "String.padStart expects 1 or 2 arguments (targetLength [, padString])",
                    );
                }
                let target_len = match self.eval_string_int_arg(
                    &dot_call.arguments[0],
                    "padStart",
                    "targetLength",
                ) {
                    Ok(value) if value >= 0 => value as usize,
                    Ok(value) => {
                        return self.rt_err_kind(
                            "RangeError",
                            format!(
                                "String.padStart targetLength must be non-negative, got {value}"
                            ),
                        );
                    }
                    Err(error) => return error,
                };
                let pad_str = if dot_call.arguments.len() == 2 {
                    match self.eval_string_arg(&dot_call.arguments[1], "padStart", "padString") {
                        Ok(value) => value,
                        Err(error) => return error,
                    }
                } else {
                    " ".to_string()
                };
                match self.build_padded_string(&s, target_len, &pad_str, true) {
                    Ok(result) => EvalResult::Value(self.alloc(ObjectData::Str(result))),
                    Err(error) => error,
                }
            }

            "padEnd" => {
                if !(1..=2).contains(&dot_call.arguments.len()) {
                    return self.rt_err_kind(
                        "TypeError",
                        "String.padEnd expects 1 or 2 arguments (targetLength [, padString])",
                    );
                }
                let target_len = match self.eval_string_int_arg(
                    &dot_call.arguments[0],
                    "padEnd",
                    "targetLength",
                ) {
                    Ok(value) if value >= 0 => value as usize,
                    Ok(value) => {
                        return self.rt_err_kind(
                            "RangeError",
                            format!("String.padEnd targetLength must be non-negative, got {value}"),
                        );
                    }
                    Err(error) => return error,
                };
                let pad_str = if dot_call.arguments.len() == 2 {
                    match self.eval_string_arg(&dot_call.arguments[1], "padEnd", "padString") {
                        Ok(value) => value,
                        Err(error) => return error,
                    }
                } else {
                    " ".to_string()
                };
                match self.build_padded_string(&s, target_len, &pad_str, false) {
                    Ok(result) => EvalResult::Value(self.alloc(ObjectData::Str(result))),
                    Err(error) => error,
                }
            }

            "slice" => {
                if dot_call.arguments.len() > 2 {
                    return self.rt_err_kind(
                        "TypeError",
                        "String.slice expects 0, 1 or 2 arguments (start [, end])",
                    );
                }
                let chars: Vec<char> = s.chars().collect();
                let slen = chars.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_string_int_arg(&dot_call.arguments[0], "slice", "start") {
                        Ok(value) => value,
                        Err(error) => return error,
                    }
                } else {
                    0
                };
                let end_i = if dot_call.arguments.len() == 2 {
                    match self.eval_string_int_arg(&dot_call.arguments[1], "slice", "end") {
                        Ok(value) => value,
                        Err(error) => return error,
                    }
                } else {
                    slen
                };
                let start = (if start_i < 0 {
                    (slen + start_i).max(0)
                } else {
                    start_i.min(slen)
                }) as usize;
                let end = (if end_i < 0 {
                    (slen + end_i).max(0)
                } else {
                    end_i.min(slen)
                }) as usize;
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

            _ => self.rt_err_kind(
                "ReferenceError",
                format!("Unknown string method '{}'", dot_call.method),
            ),
        }
    }

    // ── Argument extraction helpers ───────────────────────────────────────────

    fn eval_string_arg(
        &mut self,
        expr: &ast::Expression,
        method: &str,
        parameter: &str,
    ) -> Result<String, EvalResult> {
        let value = match self.eval_expression(expr) {
            EvalResult::Value(value) => value,
            other => return Err(other),
        };
        match self.resolve(value).cloned() {
            Some(ObjectData::Str(value)) => Ok(value),
            _ => Err(self.rt_err_kind(
                "TypeError",
                format!("String.{method}: {parameter} must be a string"),
            )),
        }
    }

    fn eval_string_int_arg(
        &mut self,
        expr: &ast::Expression,
        method: &str,
        parameter: &str,
    ) -> Result<i64, EvalResult> {
        let value = match self.eval_expression(expr) {
            EvalResult::Value(value) => value,
            other => return Err(other),
        };
        match self.resolve(value) {
            Some(ObjectData::Integer(value)) => Ok(*value),
            _ => Err(self.rt_err_kind(
                "TypeError",
                format!("String.{method}: {parameter} must be an int"),
            )),
        }
    }

    fn build_padded_string(
        &mut self,
        source: &str,
        target_len: usize,
        pad: &str,
        at_start: bool,
    ) -> Result<String, EvalResult> {
        let source_len = source.chars().count();
        if source_len >= target_len || pad.is_empty() {
            return Ok(source.to_string());
        }
        if target_len > MAX_PADDED_STRING_CHARS {
            return Err(self.fatal_err_kind(
                "ResourceError",
                format!(
                    "String padding target length {target_len} exceeds maximum ({MAX_PADDED_STRING_CHARS})"
                ),
            ));
        }

        let needed = target_len - source_len;
        let pad_chars: Vec<char> = pad.chars().collect();
        let pad_len = pad_chars.len();
        let repeated_len = needed.div_ceil(pad_len) * pad_len;
        let offset = if at_start { repeated_len - needed } else { 0 };

        let mut fill = String::new();
        if fill.try_reserve_exact(needed.saturating_mul(4)).is_err() {
            return Err(self.fatal_err_kind("ResourceError", "String padding allocation failed"));
        }
        for index in 0..needed {
            fill.push(pad_chars[(offset + index) % pad_len]);
        }

        let result_bytes = match source.len().checked_add(fill.len()) {
            Some(length) => length,
            None => {
                return Err(self.fatal_err_kind("ResourceError", "String padding size overflow"));
            }
        };
        let mut result = String::new();
        if result.try_reserve_exact(result_bytes).is_err() {
            return Err(self.fatal_err_kind("ResourceError", "String padding allocation failed"));
        }
        if at_start {
            result.push_str(&fill);
            result.push_str(source);
        } else {
            result.push_str(source);
            result.push_str(&fill);
        }
        Ok(result)
    }
}
