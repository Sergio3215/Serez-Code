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
                    eprintln!("❌ ERROR: Math.clamp(x, min, max) requires 3 arguments");
                    return EvalResult::Error;
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
                    _ => { eprintln!("❌ ERROR: Math.clamp requires numeric arguments"); EvalResult::Error }
                }
            }
            "sign" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Math.sign(x) requires 1 argument");
                    return EvalResult::Error;
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
                    _ => { eprintln!("❌ ERROR: Math.sign requires a numeric argument"); EvalResult::Error }
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
                eprintln!("❌ ERROR: Unknown Math method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── File namespace ─────────────────────────────────────────────────────────

    pub(super) fn eval_file_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "exists" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.exists(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.exists requires a string path"); return EvalResult::Error; }
                };
                let exists = std::path::Path::new(&path).exists();
                EvalResult::Value(self.alloc(ObjectData::Boolean(exists)))
            }
            "read" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.read(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.read requires a string path"); return EvalResult::Error; }
                };
                const MAX_FILE_READ: u64 = 256 * 1024 * 1024; // 256 MB
                match std::fs::metadata(&path) {
                    Ok(meta) if meta.len() > MAX_FILE_READ => {
                        eprintln!("❌ ERROR: File '{}' exceeds maximum read size of 256 MB", path);
                        return EvalResult::Error;
                    }
                    Err(e) => { eprintln!("❌ ERROR: File error reading '{}': {}", path, e); return EvalResult::Error; }
                    _ => {}
                }
                match std::fs::read_to_string(&path) {
                    Ok(content) => EvalResult::Value(self.alloc(ObjectData::Str(content))),
                    Err(e) => { eprintln!("❌ ERROR: File error reading '{}': {}", path, e); EvalResult::Error }
                }
            }
            "write" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: File.write(path, content) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let cr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.write path must be a string"); return EvalResult::Error; }
                };
                let content = self.display(cr);
                match std::fs::write(&path, &content) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: File error writing '{}': {}", path, e); EvalResult::Error }
                }
            }
            "create" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.create(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.create requires a string path"); return EvalResult::Error; }
                };
                // touch: create if not exists, leave untouched if exists
                if !std::path::Path::new(&path).exists() {
                    if let Err(e) = std::fs::File::create(&path) {
                        eprintln!("❌ ERROR: File error creating '{}': {}", path, e);
                        return EvalResult::Error;
                    }
                }
                EvalResult::Value(self.null_ref)
            }
            "read_asBinary" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.read_asBinary(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.read_asBinary requires a string path"); return EvalResult::Error; }
                };
                const MAX_BINARY_READ: u64 = 256 * 1024 * 1024; // 256 MB
                match std::fs::metadata(&path) {
                    Ok(meta) if meta.len() > MAX_BINARY_READ => {
                        eprintln!("❌ ERROR: File '{}' exceeds maximum read size of 256 MB", path);
                        return EvalResult::Error;
                    }
                    Err(e) => { eprintln!("❌ ERROR: File error reading binary '{}': {}", path, e); return EvalResult::Error; }
                    _ => {}
                }
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let refs: Vec<ObjectRef> = bytes.iter()
                            .map(|&b| self.alloc(ObjectData::Integer(b as i64)))
                            .collect();
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements: refs }))
                    }
                    Err(e) => { eprintln!("❌ ERROR: File error reading binary '{}': {}", path, e); EvalResult::Error }
                }
            }
            "write_asBinary" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: File.write_asBinary(path, bytes) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let br = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.write_asBinary path must be a string"); return EvalResult::Error; }
                };
                let bytes_data = match self.resolve(br).cloned() {
                    Some(ObjectData::Array { elements, .. }) => elements,
                    _ => { eprintln!("❌ ERROR: File.write_asBinary bytes must be an array"); return EvalResult::Error; }
                };
                let mut buf: Vec<u8> = Vec::with_capacity(bytes_data.len());
                for r in bytes_data {
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(b)) if b >= 0 && b <= 255 => buf.push(b as u8),
                        _ => { eprintln!("❌ ERROR: File.write_asBinary: each byte must be int 0-255"); return EvalResult::Error; }
                    }
                }
                match std::fs::write(&path, &buf) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: File error writing binary '{}': {}", path, e); EvalResult::Error }
                }
            }
            _ => {
                eprintln!("❌ ERROR: Unknown File method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── JSON namespace ─────────────────────────────────────────────────────────

    pub(super) fn eval_json_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "stringify" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: JSON.stringify(value) requires 1 argument");
                    return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let owned = self.extract(vr);
                let json = json_stringify_owned(&owned);
                EvalResult::Value(self.alloc(ObjectData::Str(json)))
            }
            "parse" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: JSON.parse(string) requires 1 argument");
                    return EvalResult::Error;
                }
                let sr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let s = match self.resolve(sr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: JSON.parse requires a string"); return EvalResult::Error; }
                };
                match json_parse(&s) {
                    Ok(owned) => EvalResult::Value(self.plant_global(owned)),
                    Err(e) => { eprintln!("❌ ERROR: JSON.parse error: {}", e); EvalResult::Error }
                }
            }
            _ => {
                eprintln!("❌ ERROR: Unknown JSON method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Set methods ────────────────────────────────────────────────────────────

}
