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
    pub(super) fn eval_assert(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.is_empty() || args.len() > 2 {
            eprintln!("❌ ERROR: assert(condition) or assert(condition, message)");
            return EvalResult::Error;
        }
        let cond_ref = match self.eval_expression(&args[0]) {
            EvalResult::Value(v) => v,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let is_true = matches!(self.resolve(cond_ref), Some(ObjectData::Boolean(true)));
        if !is_true {
            let msg = if args.len() == 2 {
                match self.eval_expression(&args[1]) {
                    EvalResult::Value(r) => self.display(r),
                    _ => "Assertion failed".to_string(),
                }
            } else {
                "Assertion failed".to_string()
            };
            let msg_ref = self.alloc(ObjectData::Str(msg));
            EvalResult::Throw(msg_ref)
        } else {
            EvalResult::Value(self.null_ref)
        }
    }

    pub(super) fn eval_type_of(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: type_of expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(v) => v,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let type_name = match self.resolve(r) {
            Some(ObjectData::Integer(_))  => "int",
            Some(ObjectData::Decimal(_))  => "decimal",
            Some(ObjectData::Boolean(_))  => "bool",
            Some(ObjectData::Str(_))      => "string",
            Some(ObjectData::Array { .. }) => "array",
            Some(ObjectData::Dict { .. }) => "dict",
            Some(ObjectData::Function { .. }) => "function",
            Some(ObjectData::Instance { class_name, .. }) => {
                // class_name vive en la arena, necesitamos clonar antes de alloc
                let name = class_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return EvalResult::Value(s);
            }
            Some(ObjectData::Null) | None => "null",
            Some(ObjectData::EnumVariant { enum_name, .. }) => {
                let name = enum_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return EvalResult::Value(s);
            }
            Some(ObjectData::Set { .. }) => "Set",
        };
        EvalResult::Value(self.alloc(ObjectData::Str(type_name.to_string())))
    }

    pub(super) fn eval_parse_int(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: parseInt expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Integer(i))),
            Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Integer(d as i64))),
            Some(ObjectData::Str(s)) => match s.trim().parse::<i64>() {
                Ok(n) => EvalResult::Value(self.alloc(ObjectData::Integer(n))),
                Err(_) => {
                    eprintln!("❌ ERROR: parseInt: cannot parse '{}' as int", s);
                    EvalResult::Error
                }
            },
            _ => { eprintln!("❌ ERROR: parseInt: unsupported type"); EvalResult::Error }
        }
    }

    pub(super) fn eval_parse_decimal(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: parseDecimal expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Decimal(i as f64))),
            Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Decimal(d))),
            Some(ObjectData::Str(s)) => match s.trim().parse::<f64>() {
                Ok(n) => EvalResult::Value(self.alloc(ObjectData::Decimal(n))),
                Err(_) => {
                    eprintln!("❌ ERROR: parseDecimal: cannot parse '{}' as decimal", s);
                    EvalResult::Error
                }
            },
            _ => { eprintln!("❌ ERROR: parseDecimal: unsupported type"); EvalResult::Error }
        }
    }

    // ── Math built-ins ────────────────────────────────────────────────────────

    pub(super) fn eval_math_builtin(&mut self, name: &str, args: &[ast::Expression]) -> EvalResult {
        // Helper: resolve one numeric argument to f64
        let resolve_num = |evaluator: &mut Self, expr: &ast::Expression| -> Option<f64> {
            match evaluator.eval_expression(expr) {
                EvalResult::Value(r) => match evaluator.resolve(r).cloned() {
                    Some(ObjectData::Integer(i)) => Some(i as f64),
                    Some(ObjectData::Decimal(d)) => Some(d),
                    _ => { eprintln!("❌ ERROR: Math function '{}' expects numeric argument", name); None }
                },
                _ => None,
            }
        };

        match name {
            // --- Single-argument ---
            "abs" => {
                if args.len() != 1 { eprintln!("❌ ERROR: abs() expects 1 argument"); return EvalResult::Error; }
                let r = match self.eval_expression(&args[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Integer(i.abs()))),
                    Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Decimal(d.abs()))),
                    _ => { eprintln!("❌ ERROR: abs() expects a numeric argument"); EvalResult::Error }
                }
            }
            "sqrt" => {
                if args.len() != 1 { eprintln!("❌ ERROR: sqrt() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v < 0.0 { eprintln!("❌ ERROR: sqrt() of negative number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.sqrt())))
            }
            "floor" => {
                if args.len() != 1 { eprintln!("❌ ERROR: floor() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.floor() as i64)))
            }
            "ceil" => {
                if args.len() != 1 { eprintln!("❌ ERROR: ceil() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.ceil() as i64)))
            }
            "round" => {
                if args.len() != 1 { eprintln!("❌ ERROR: round() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.round() as i64)))
            }
            "log" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.ln())))
            }
            "log2" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log2() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log2() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log2())))
            }
            "log10" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log10() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log10() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log10())))
            }
            // --- Two-argument ---
            "min" => {
                if args.is_empty() { eprintln!("❌ ERROR: min() expects at least 1 argument"); return EvalResult::Error; }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => { vals.push(i as f64); int_vals.push(i); }
                        Some(ObjectData::Decimal(d)) => { vals.push(d); all_int = false; }
                        _ => { eprintln!("❌ ERROR: min() expects numeric arguments"); return EvalResult::Error; }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    EvalResult::Value(self.alloc(ObjectData::Integer(*int_vals.iter().min().unwrap())))
                } else {
                    let m = vals.iter().cloned().fold(f64::INFINITY, f64::min);
                    EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
                }
            }
            "max" => {
                if args.is_empty() { eprintln!("❌ ERROR: max() expects at least 1 argument"); return EvalResult::Error; }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => { vals.push(i as f64); int_vals.push(i); }
                        Some(ObjectData::Decimal(d)) => { vals.push(d); all_int = false; }
                        _ => { eprintln!("❌ ERROR: max() expects numeric arguments"); return EvalResult::Error; }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    EvalResult::Value(self.alloc(ObjectData::Integer(*int_vals.iter().max().unwrap())))
                } else {
                    let m = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
                }
            }
            "pow" => {
                if args.len() != 2 { eprintln!("❌ ERROR: pow() expects 2 arguments (base, exp)"); return EvalResult::Error; }
                let base = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                let exp  = match resolve_num(self, &args[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(base.powf(exp))))
            }
            "sin" => {
                if args.len() != 1 { eprintln!("❌ ERROR: sin() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.sin())))
            }
            "cos" => {
                if args.len() != 1 { eprintln!("❌ ERROR: cos() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.cos())))
            }
            "tan" => {
                if args.len() != 1 { eprintln!("❌ ERROR: tan() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.tan())))
            }
            "asin" => {
                if args.len() != 1 { eprintln!("❌ ERROR: asin() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.asin())))
            }
            "acos" => {
                if args.len() != 1 { eprintln!("❌ ERROR: acos() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.acos())))
            }
            "atan" => {
                if args.len() != 1 { eprintln!("❌ ERROR: atan() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.atan())))
            }
            "atan2" => {
                if args.len() != 2 { eprintln!("❌ ERROR: atan2() expects 2 arguments"); return EvalResult::Error; }
                let y = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                let x = match resolve_num(self, &args[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(y.atan2(x))))
            }
            "trunc" => {
                if args.len() != 1 { eprintln!("❌ ERROR: trunc() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.trunc() as i64)))
            }
            "exp" => {
                if args.len() != 1 { eprintln!("❌ ERROR: exp() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.exp())))
            }
            _ => { eprintln!("❌ ERROR: Unknown math function '{}'", name); EvalResult::Error }
        }
    }

    pub(super) fn eval_read_line(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() > 1 {
            eprintln!("❌ ERROR: readLine expects 0 or 1 argument");
            return EvalResult::Error;
        }
        if let Some(prompt_expr) = args.first() {
            match self.eval_expression(prompt_expr) {
                EvalResult::Value(r) => {
                    let prompt = self.display(r);
                    print!("{}", prompt);
                    let _ = io::stdout().flush();
                }
                EvalResult::Throw(v) => return EvalResult::Throw(v),
                _ => return EvalResult::Error,
            }
        }
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
                EvalResult::Value(self.alloc(ObjectData::Str(trimmed)))
            }
            Err(e) => {
                eprintln!("❌ ERROR: readLine: failed to read from stdin — {}", e);
                EvalResult::Error
            }
        }
    }

    // ── Interface / Class instantiation ──────────────────────────────────────

}
