#![allow(unused_imports)]
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use super::{EvalResult, StoredClass, CallFrame, type_matches, obj_data_to_key_str,
            obj_data_eq, format_decimal, json_stringify_owned, json_pretty_owned, json_parse,
            operator_to_method_name};

impl super::Evaluator {
    pub(super) fn eval_math_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "PI" => EvalResult::Value(self.alloc(ObjectData::Decimal(std::f64::consts::PI))),
            "E"  => EvalResult::Value(self.alloc(ObjectData::Decimal(std::f64::consts::E))),
            "random" => {
                let val = self.lcg_next_f64();
                EvalResult::Value(self.alloc(ObjectData::Decimal(val)))
            }
            "clamp" => {
                if dot_call.arguments.len() != 3 {
                    return self.rt_err_kind("TypeError", "Math.clamp(x, min, max) requires 3 arguments");
                }
                let x   = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let mn  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let mx  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                match (self.resolve(x).cloned(), self.resolve(mn).cloned(), self.resolve(mx).cloned()) {
                    (Some(ObjectData::Integer(xv)), Some(ObjectData::Integer(mnv)), Some(ObjectData::Integer(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Integer(xv.max(mnv).min(mxv)))),
                    (Some(ObjectData::Decimal(xv)), Some(ObjectData::Decimal(mnv)), Some(ObjectData::Decimal(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Decimal(xv.max(mnv).min(mxv)))),
                    (Some(ObjectData::Integer(xv)), Some(ObjectData::Integer(mnv)), Some(ObjectData::Decimal(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Decimal((xv as f64).max(mnv as f64).min(mxv)))),
                    _ => self.rt_err_kind("TypeError", "Math.clamp requires numeric arguments"),
                }
            }
            "sign" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "Math.sign(x) requires 1 argument");
                }
                let xr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                match self.resolve(xr).cloned() {
                    Some(ObjectData::Integer(v)) => {
                        let s = if v > 0 { 1i64 } else if v < 0 { -1 } else { 0 };
                        EvalResult::Value(self.alloc(ObjectData::Integer(s)))
                    }
                    Some(ObjectData::Decimal(v)) => {
                        let s = if v > 0.0 { 1i64 } else if v < 0.0 { -1 } else { 0 };
                        EvalResult::Value(self.alloc(ObjectData::Integer(s)))
                    }
                    _ => self.rt_err_kind("TypeError", "Math.sign requires a numeric argument"),
                }
            }
            // Delegate single-arg math functions to eval_math_builtin
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "log" | "log2" | "log10"
            | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "trunc" | "exp" => {
                self.eval_math_builtin(dot_call.method.as_str(), &dot_call.arguments)
            }
            // Multi-arg: min, max, pow, atan2
            "min" | "max" | "pow" | "atan2" => {
                self.eval_math_builtin(dot_call.method.as_str(), &dot_call.arguments)
            }
            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("TypeError", format!("Unknown Math method '{}'", m))
            }
        }
    }

    // ── File namespace ─────────────────────────────────────────────────────────

    pub(super) fn eval_file_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "exists" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.exists(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.exists requires a string path"),
                };
                let exists = std::path::Path::new(&path).exists();
                EvalResult::Value(self.alloc(ObjectData::Boolean(exists)))
            }
            "read" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.read(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.read requires a string path"),
                };
                const MAX_FILE_READ: u64 = 256 * 1024 * 1024; // 256 MB
                match std::fs::metadata(&path) {
                    // Resource limit: stays fatal and non-catchable (DoS protection).
                    Ok(meta) if meta.len() > MAX_FILE_READ => {
                        eprintln!("❌ ERROR: File '{}' exceeds maximum read size of 256 MB", path);
                        return EvalResult::Error;
                    }
                    Err(e) => return self.rt_err_kind("IOError", format!("File error reading '{}': {}", path, e)),
                    _ => {}
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => EvalResult::Value(self.alloc(ObjectData::Str(content))),
                    Err(e) => self.rt_err_kind("IOError", format!("File error reading '{}': {}", path, e)),
                }
            }
            "write" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "File.write(path, content) requires 2 arguments");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let cr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.write path must be a string"),
                };
                let content = self.display(cr);
                match std::fs::write(&path, &content) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("File error writing '{}': {}", path, e)),
                }
            }
            "create" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.create(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.create requires a string path"),
                };
                // touch: create if not exists, leave untouched if exists
                if !std::path::Path::new(&path).exists() {
                    if let Err(e) = std::fs::File::create(&path) {
                        return self.rt_err_kind("IOError", format!("File error creating '{}': {}", path, e));
                    }
                }
                EvalResult::Value(self.null_ref)
            }
            "read_asBinary" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.read_asBinary(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.read_asBinary requires a string path"),
                };
                const MAX_BINARY_READ: u64 = 256 * 1024 * 1024; // 256 MB
                match std::fs::metadata(&path) {
                    // Resource limit: stays fatal and non-catchable (DoS protection).
                    Ok(meta) if meta.len() > MAX_BINARY_READ => {
                        eprintln!("❌ ERROR: File '{}' exceeds maximum read size of 256 MB", path);
                        return EvalResult::Error;
                    }
                    Err(e) => return self.rt_err_kind("IOError", format!("File error reading binary '{}': {}", path, e)),
                    _ => {}
                }
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let owned: Vec<OwnedValue> = bytes.iter()
                            .map(|&b| OwnedValue::Integer(b as i64))
                            .collect();
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements: owned }))
                    }
                    Err(e) => self.rt_err_kind("IOError", format!("File error reading binary '{}': {}", path, e)),
                }
            }
            "write_asBinary" => {
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "File.write_asBinary(path, bytes) requires 2 arguments");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let br = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.write_asBinary path must be a string"),
                };
                let bytes_data = match self.resolve(br).cloned() {
                    Some(ObjectData::Array { elements, .. }) => elements,
                    _ => return self.rt_err_kind("TypeError", "File.write_asBinary bytes must be an array"),
                };
                let mut buf: Vec<u8> = Vec::with_capacity(bytes_data.len());
                for owned in bytes_data {
                    match owned {
                        OwnedValue::Integer(b) if b >= 0 && b <= 255 => buf.push(b as u8),
                        _ => return self.rt_err_kind("TypeError", "File.write_asBinary: each byte must be int 0-255"),
                    }
                }
                match std::fs::write(&path, &buf) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("File error writing binary '{}': {}", path, e)),
                }
            }
            "listDir" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.listDir(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.listDir requires a string path"),
                };
                match std::fs::read_dir(&path) {
                    Ok(entries) => {
                        let owned: Vec<OwnedValue> = entries
                            .filter_map(|e| e.ok())
                            .map(|e| {
                                let name = e.file_name().to_string_lossy().to_string();
                                OwnedValue::Str(name)
                            })
                            .collect();
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("string".to_string()), elements: owned }))
                    }
                    Err(e) => self.rt_err_kind("IOError", format!("File.listDir '{}' failed: {}", path, e)),
                }
            }
            "mkdir" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.mkdir(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.mkdir requires a string path"),
                };
                match std::fs::create_dir_all(&path) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("File.mkdir '{}' failed: {}", path, e)),
                }
            }
            "stat" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.stat(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.stat requires a string path"),
                };
                match std::fs::metadata(&path) {
                    Ok(meta) => {
                        let size = meta.len() as i64;
                        let is_dir = meta.is_dir();
                        let modified = meta.modified()
                            .ok()
                            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                            .map(|d| d.as_millis() as i64)
                            .unwrap_or(-1);
                        use crate::region::OwnedValue;
                        EvalResult::Value(self.alloc(ObjectData::Instance {
                            class_name: "FileStat".to_string(),
                            fields: vec![
                                ("size".to_string(),     OwnedValue::Integer(size)),
                                ("modified".to_string(), OwnedValue::Integer(modified)),
                                ("isDir".to_string(),    OwnedValue::Boolean(is_dir)),
                            ],
                        }))
                    }
                    Err(e) => self.rt_err_kind("IOError", format!("File.stat '{}' failed: {}", path, e)),
                }
            }
            "delete" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: File.delete requires an `unsafe {{ }}` block — it permanently removes files");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "File.delete(path) requires 1 argument");
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.delete requires a string path"),
                };
                let p = std::path::Path::new(&path);
                let result = if p.is_dir() { std::fs::remove_dir_all(p) } else { std::fs::remove_file(p) };
                match result {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("File.delete '{}' failed: {}", path, e)),
                }
            }
            "rename" => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: File.rename requires an `unsafe {{ }}` block — it modifies the filesystem");
                    return EvalResult::Error;
                }
                if dot_call.arguments.len() != 2 {
                    return self.rt_err_kind("TypeError", "File.rename(from, to) requires 2 arguments");
                }
                let fr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let tr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let from = match self.resolve(fr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.rename: 'from' must be a string"),
                };
                let to = match self.resolve(tr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "File.rename: 'to' must be a string"),
                };
                match std::fs::rename(&from, &to) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => self.rt_err_kind("IOError", format!("File.rename '{}' → '{}' failed: {}", from, to, e)),
                }
            }
            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("TypeError", format!("Unknown File method '{}'", m))
            }
        }
    }

    // ── JSON namespace ─────────────────────────────────────────────────────────

    pub(super) fn eval_json_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "stringify" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "JSON.stringify(value) requires 1 argument");
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let owned = self.extract(vr);
                let json = json_stringify_owned(&owned);
                EvalResult::Value(self.alloc(ObjectData::Str(json)))
            }
            "parse" => {
                if dot_call.arguments.len() != 1 {
                    return self.rt_err_kind("TypeError", "JSON.parse(string) requires 1 argument");
                }
                let sr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let s = match self.resolve(sr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => return self.rt_err_kind("TypeError", "JSON.parse requires a string"),
                };
                match json_parse(&s) {
                    Ok(owned) => EvalResult::Value(self.plant_global(owned)),
                    Err(e) => self.rt_err_kind("JsonError", format!("JSON.parse error: {}", e)),
                }
            }
            "pretty" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    return self.rt_err_kind("TypeError", "JSON.pretty(value, [indent]) requires 1 or 2 arguments");
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let owned = self.extract(vr);
                // Optional second arg: spaces per level (default 2).
                let indent = if dot_call.arguments.len() == 2 {
                    let ir = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                    match self.resolve(ir) {
                        Some(ObjectData::Integer(n)) if *n >= 0 => *n as usize,
                        _ => return self.rt_err_kind("TypeError", "JSON.pretty indent must be a non-negative integer"),
                    }
                } else { 2 };
                // A raw string (e.g. a fetch body) is parsed as JSON first so its
                // structure can be re-indented; non-JSON strings are left as-is.
                let target: OwnedValue = if let OwnedValue::Str(s) = &owned {
                    json_parse(s).unwrap_or_else(|_| owned.clone())
                } else {
                    owned
                };
                let json = json_pretty_owned(&target, indent);
                EvalResult::Value(self.alloc(ObjectData::Str(json)))
            }
            _ => {
                let m = dot_call.method.clone();
                self.rt_err_kind("TypeError", format!("Unknown JSON method '{}'", m))
            }
        }
    }

    // ── Set methods ────────────────────────────────────────────────────────────

}
