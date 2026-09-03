#![allow(unused_imports)]
use super::{
    CallFrame, EvalResult, StoredClass, format_decimal, json_parse, json_stringify_owned,
    obj_data_eq, obj_data_to_key_str, operator_to_method_name, type_matches,
};
use super::{ExecutionFlow, RuntimeFailure};
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

/// An HTTP response, normalised so `fetch` reads the same whether the bytes came
/// from ureq or from the browser. Header names are lowercased.
pub(super) struct FetchResponse {
    pub status: u16,
    pub status_text: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

impl super::Evaluator {
    pub(super) fn eval_assert(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.is_empty() || args.len() > 2 {
            return self.rt_err_kind(
                "RuntimeError",
                "assert(condition) or assert(condition, message)",
            );
        }
        let cond_ref = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(v)) => v,
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };
        let is_true = matches!(self.resolve(cond_ref), Some(ObjectData::Boolean(true)));
        if !is_true {
            let msg = if args.len() == 2 {
                match self.eval_expression(&args[1]) {
                    Ok(ExecutionFlow::Value(r)) => self.display(r),
                    _ => "Assertion failed".to_string(),
                }
            } else {
                "Assertion failed".to_string()
            };
            let msg_ref = self.alloc(ObjectData::Str(msg));
            Ok(ExecutionFlow::Throw(msg_ref))
        } else {
            Ok(ExecutionFlow::Value(self.null_ref))
        }
    }

    pub(super) fn eval_type_of(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            return self.rt_err_kind("TypeError", "type_of expects 1 argument");
        }
        let r = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(v)) => v,
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };
        let type_name = match self.resolve(r) {
            Some(ObjectData::Integer(_)) => "int",
            Some(ObjectData::Decimal(_)) => "decimal",
            Some(ObjectData::Dec(_)) => "dec",
            Some(ObjectData::Boolean(_)) => "bool",
            Some(ObjectData::Str(_)) => "string",
            Some(ObjectData::Array { .. }) => "array",
            Some(ObjectData::Dict { .. }) => "dict",
            Some(ObjectData::Function { .. }) => "function",
            Some(ObjectData::Instance { class_name, .. }) => {
                // class_name vive en la arena, necesitamos clonar antes de alloc
                let name = class_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return Ok(ExecutionFlow::Value(s));
            }
            Some(ObjectData::Ptr(_)) => "ptr",
            Some(ObjectData::Null) | None => "null",
            Some(ObjectData::EnumVariant { enum_name, .. }) => {
                let name = enum_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return Ok(ExecutionFlow::Value(s));
            }
            Some(ObjectData::Set { .. }) => "Set",
            Some(ObjectData::Tensor { .. }) => "Tensor",
            Some(ObjectData::DateTime { .. }) => "DateTime",
            // A DateField behaves as an int under operators.
            Some(ObjectData::DateField { .. }) => "int",
        };
        Ok(ExecutionFlow::Value(
            self.alloc(ObjectData::Str(type_name.to_string())),
        ))
    }

    pub(super) fn eval_parse_int(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            return self.rt_err_kind("TypeError", "parseInt expects 1 argument");
        }
        let r = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(r)) => r,
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => {
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(i))))
            }
            Some(ObjectData::Decimal(d)) => {
                if !d.is_finite() || d > i64::MAX as f64 || d < i64::MIN as f64 {
                    return self.rt_err_kind(
                        "RuntimeError",
                        "parseInt: decimal value is out of int range or not finite",
                    );
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Integer(d as i64)),
                ))
            }
            Some(ObjectData::Str(s)) => match s.trim().parse::<i64>() {
                Ok(n) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(n)))),
                Err(_) => self.rt_err_kind(
                    "RuntimeError",
                    format!("parseInt: cannot parse '{}' as int", s),
                ),
            },
            _ => self.rt_err_kind("RuntimeError", "parseInt: unsupported type"),
        }
    }

    pub(super) fn eval_parse_decimal(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            return self.rt_err_kind("TypeError", "parseDecimal expects 1 argument");
        }
        let r = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(r)) => r,
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => Ok(ExecutionFlow::Value(
                self.alloc(ObjectData::Decimal(i as f64)),
            )),
            Some(ObjectData::Decimal(d)) => {
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(d))))
            }
            Some(ObjectData::Str(s)) => match s.trim().parse::<f64>() {
                Ok(n) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(n)))),
                Err(_) => self.rt_err_kind(
                    "RuntimeError",
                    format!("parseDecimal: cannot parse '{}' as decimal", s),
                ),
            },
            _ => self.rt_err_kind("RuntimeError", "parseDecimal: unsupported type"),
        }
    }

    // ── Math built-ins ────────────────────────────────────────────────────────

    /// Evaluate one Math argument without collapsing its control/error signal.
    ///
    /// The old `Option<f64>` helper mapped `Throw` and structured runtime errors
    /// to `None`; callers then returned a fresh empty `Err(RuntimeFailure)`, losing
    /// the user exception or its diagnostic payload. Only an evaluated,
    /// non-numeric value is a new Math `TypeError`.
    fn eval_math_number(
        &mut self,
        function_name: &str,
        expr: &ast::Expression,
    ) -> Result<f64, EvalResult> {
        let value_ref = match self.eval_expression(expr) {
            Ok(ExecutionFlow::Value(value_ref)) => value_ref,
            other => return Err(other),
        };

        match self.resolve(value_ref).cloned() {
            Some(ObjectData::Integer(value)) => Ok(value as f64),
            Some(ObjectData::Decimal(value)) => Ok(value),
            Some(_) => Err(self.rt_err_kind(
                "TypeError",
                format!("Math function '{function_name}' expects numeric argument"),
            )),
            None => Err(self.rt_err_kind(
                "InternalError",
                format!("Math function '{function_name}' received an invalid value reference"),
            )),
        }
    }

    pub(super) fn eval_math_builtin(&mut self, name: &str, args: &[ast::Expression]) -> EvalResult {
        match name {
            // --- Single-argument ---
            "abs" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "abs() expects 1 argument");
                }
                let r = match self.eval_expression(&args[0]) {
                    Ok(ExecutionFlow::Value(r)) => r,
                    other => return other,
                };
                match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(i)) => match i.checked_abs() {
                        Some(v) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(v)))),
                        None => self.rt_err_kind(
                            "RuntimeError",
                            "abs() overflow (i64::MIN has no positive representation)",
                        ),
                    },
                    Some(ObjectData::Decimal(d)) => Ok(ExecutionFlow::Value(
                        self.alloc(ObjectData::Decimal(d.abs())),
                    )),
                    _ => self.rt_err_kind("TypeError", "abs() expects a numeric argument"),
                }
            }
            "sqrt" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "sqrt() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v < 0.0 {
                    return self.rt_err_kind("RuntimeError", "sqrt() of negative number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.sqrt())),
                ))
            }
            "floor" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "floor() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v.is_nan() || v.is_infinite() {
                    return self
                        .rt_err_kind("TypeError", "floor() argument must be a finite number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Integer(v.floor() as i64)),
                ))
            }
            "ceil" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "ceil() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v.is_nan() || v.is_infinite() {
                    return self
                        .rt_err_kind("TypeError", "ceil() argument must be a finite number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Integer(v.ceil() as i64)),
                ))
            }
            "round" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "round() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v.is_nan() || v.is_infinite() {
                    return self
                        .rt_err_kind("TypeError", "round() argument must be a finite number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Integer(v.round() as i64)),
                ))
            }
            "log" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "log() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v <= 0.0 {
                    return self.rt_err_kind("RuntimeError", "log() of non-positive number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.ln())),
                ))
            }
            "log2" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "log2() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v <= 0.0 {
                    return self.rt_err_kind("RuntimeError", "log2() of non-positive number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.log2())),
                ))
            }
            "log10" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "log10() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v <= 0.0 {
                    return self.rt_err_kind("RuntimeError", "log10() of non-positive number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.log10())),
                ))
            }
            // --- Two-argument ---
            "min" => {
                if args.is_empty() {
                    return self.rt_err_kind("TypeError", "min() expects at least 1 argument");
                }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) {
                        Ok(ExecutionFlow::Value(r)) => r,
                        other => return other,
                    };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => {
                            vals.push(i as f64);
                            int_vals.push(i);
                        }
                        Some(ObjectData::Decimal(d)) => {
                            vals.push(d);
                            all_int = false;
                        }
                        _ => {
                            return self
                                .rt_err_kind("TypeError", "min() expects numeric arguments");
                        }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    match int_vals.iter().min().copied() {
                        Some(value) => {
                            Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(value))))
                        }
                        None => self
                            .rt_err_kind("InternalError", "min() lost its non-empty argument list"),
                    }
                } else {
                    let m = vals.iter().cloned().fold(f64::INFINITY, f64::min);
                    Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(m))))
                }
            }
            "max" => {
                if args.is_empty() {
                    return self.rt_err_kind("TypeError", "max() expects at least 1 argument");
                }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) {
                        Ok(ExecutionFlow::Value(r)) => r,
                        other => return other,
                    };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => {
                            vals.push(i as f64);
                            int_vals.push(i);
                        }
                        Some(ObjectData::Decimal(d)) => {
                            vals.push(d);
                            all_int = false;
                        }
                        _ => {
                            return self
                                .rt_err_kind("TypeError", "max() expects numeric arguments");
                        }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    match int_vals.iter().max().copied() {
                        Some(value) => {
                            Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(value))))
                        }
                        None => self
                            .rt_err_kind("InternalError", "max() lost its non-empty argument list"),
                    }
                } else {
                    let m = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(m))))
                }
            }
            "pow" => {
                if args.len() != 2 {
                    return self.rt_err_kind("TypeError", "pow() expects 2 arguments (base, exp)");
                }
                let base = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                let exp = match self.eval_math_number(name, &args[1]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(base.powf(exp))),
                ))
            }
            "sin" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "sin() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.sin())),
                ))
            }
            "cos" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "cos() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.cos())),
                ))
            }
            "tan" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "tan() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.tan())),
                ))
            }
            "asin" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "asin() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if !(-1.0..=1.0).contains(&v) {
                    return self.rt_err_kind("TypeError", "asin() argument must be in [-1, 1]");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.asin())),
                ))
            }
            "acos" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "acos() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if !(-1.0..=1.0).contains(&v) {
                    return self.rt_err_kind("TypeError", "acos() argument must be in [-1, 1]");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.acos())),
                ))
            }
            "atan" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "atan() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.atan())),
                ))
            }
            "atan2" => {
                if args.len() != 2 {
                    return self.rt_err_kind("TypeError", "atan2() expects 2 arguments");
                }
                let y = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                let x = match self.eval_math_number(name, &args[1]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(y.atan2(x))),
                ))
            }
            "trunc" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "trunc() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                if v.is_nan() || v.is_infinite() {
                    return self
                        .rt_err_kind("TypeError", "trunc() argument must be a finite number");
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Integer(v.trunc() as i64)),
                ))
            }
            "exp" => {
                if args.len() != 1 {
                    return self.rt_err_kind("TypeError", "exp() expects 1 argument");
                }
                let v = match self.eval_math_number(name, &args[0]) {
                    Ok(v) => v,
                    Err(signal) => return signal,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Decimal(v.exp())),
                ))
            }
            _ => self.rt_err_kind("TypeError", format!("Unknown math function '{}'", name)),
        }
    }

    pub(super) fn eval_read_line(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() > 1 {
            return self.rt_err_kind("TypeError", "readLine expects 0 or 1 argument");
        }
        if let Some(prompt_expr) = args.first() {
            match self.eval_expression(prompt_expr) {
                Ok(ExecutionFlow::Value(r)) => {
                    let prompt = self.display(r);
                    print!("{}", prompt);
                    let _ = io::stdout().flush();
                }
                Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
                _ => return Err(RuntimeFailure),
            }
        }
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(trimmed))))
            }
            Err(e) => self.rt_err_kind(
                "RuntimeError",
                format!("readLine: failed to read from stdin — {}", e),
            ),
        }
    }

    // ── Interface / Class instantiation ──────────────────────────────────────

    // ── Native: fetch ─────────────────────────────────────────────────────────

    // fetch(url, [method], [body], [options]) — general-purpose HTTP client.
    //   method/body are the string arguments after url; options is a dict:
    //     { headers: <dict>, timeout: <int secs>, full: <bool>, binary: <bool> }
    //   Default: returns the body (string), throws on HTTP status >= 400.
    //   { full: true }   → returns Dict<string, any> { status, ok, statusText, headers, body }
    //                      and does NOT throw on status.
    //   { binary: true } → body is returned as a byte array [int] instead of a string.
    pub(super) fn eval_fetch(&mut self, args: &[ast::Expression]) -> EvalResult {
        // Gated by lockdown since DEC-M7-006. It used to be exempt, on the
        // reasoning that lockdown was about the machine's own capabilities and
        // the network was a separate question — but the request goes out from the
        // host's network position, which is the usual SSRF shape: cloud metadata
        // endpoints, services bound to localhost, the host as an open relay. A
        // mode called "untrusted source" that leaves that open is a name doing
        // more work than the code.
        //
        // Blocked by default under lockdown, reachable only through an explicit
        // allowlist the *embedder* sets. The check is below rather than here,
        // because it needs the URL — and it is applied again to every redirect
        // hop, or an allowed host could hand the request to a forbidden one.
        if args.is_empty() || args.len() > 4 {
            return self.rt_err_kind("RuntimeError", "fetch(url, [method], [body], [options])");
        }

        // ── arg[0]: url (required, string) ────────────────────────────────────
        let url = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(r)) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => s,
                _ => {
                    let msg = self.alloc(ObjectData::Str(
                        "❌ fetch: url must be a string".to_string(),
                    ));
                    return Ok(ExecutionFlow::Throw(msg));
                }
            },
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };

        // ── args[1..]: 1st string = method, 2nd string = body, dict = options ──
        let mut method: Option<String> = None;
        let mut body_str = String::new();
        let mut body_set = false;
        let mut options: Option<ObjectData> = None;
        for arg in &args[1..] {
            let r = match self.eval_expression(arg) {
                Ok(ExecutionFlow::Value(r)) => r,
                Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
                _ => return Err(RuntimeFailure),
            };
            match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => {
                    if method.is_none() {
                        method = Some(s.to_uppercase());
                    } else if !body_set {
                        body_str = s;
                        body_set = true;
                    } else {
                        let msg = self.alloc(ObjectData::Str(
                            "❌ fetch: too many string arguments (expected method, body)"
                                .to_string(),
                        ));
                        return Ok(ExecutionFlow::Throw(msg));
                    }
                }
                Some(d @ ObjectData::Dict { .. }) => {
                    if options.is_some() {
                        let msg = self.alloc(ObjectData::Str(
                            "❌ fetch: options dict provided more than once".to_string(),
                        ));
                        return Ok(ExecutionFlow::Throw(msg));
                    }
                    options = Some(d);
                }
                _ => {
                    let msg = self.alloc(ObjectData::Str(
                        "❌ fetch: arguments after url must be strings (method/body) or a dict (options)".to_string()));
                    return Ok(ExecutionFlow::Throw(msg));
                }
            }
        }
        let method = method.unwrap_or_else(|| "GET".to_string());

        // ── parse options ─────────────────────────────────────────────────────
        let mut headers: Vec<(String, String)> = Vec::new();
        let mut timeout_secs: u64 = 60;
        let mut full = false;
        let mut binary = false;
        if let Some(ObjectData::Dict { entries, .. }) = &options {
            if let Some(OwnedValue::Dict {
                entries: hentries, ..
            }) = Self::fetch_dict_get(entries, "headers")
            {
                for (k, v) in hentries {
                    let name = match k {
                        OwnedValue::Str(s) => s.clone(),
                        _ => {
                            let msg = self.alloc(ObjectData::Str(
                                "❌ fetch: header names must be strings".to_string(),
                            ));
                            return Ok(ExecutionFlow::Throw(msg));
                        }
                    };
                    let value = v.display_str();
                    if name
                        .chars()
                        .chain(value.chars())
                        .any(|c| matches!(c, '\n' | '\r' | '\0'))
                    {
                        let msg = self.alloc(ObjectData::Str(format!(
                            "❌ fetch: illegal control character in header '{}'",
                            name
                        )));
                        return Ok(ExecutionFlow::Throw(msg));
                    }
                    headers.push((name, value));
                }
            }
            if let Some(OwnedValue::Integer(n)) = Self::fetch_dict_get(entries, "timeout") {
                if *n > 0 {
                    timeout_secs = *n as u64;
                }
            }
            if let Some(OwnedValue::Boolean(b)) = Self::fetch_dict_get(entries, "full") {
                full = *b;
            }
            if let Some(OwnedValue::Boolean(b)) = Self::fetch_dict_get(entries, "binary") {
                binary = *b;
            }
        }

        // ── Security validation ───────────────────────────────────────────────
        let lower = url.to_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            let msg = self.alloc(ObjectData::Str(format!(
                "❌ fetch: only http:// and https:// URLs are allowed (got: {})",
                url
            )));
            return Ok(ExecutionFlow::Throw(msg));
        }

        // Reject control characters (header injection, etc.)
        if url
            .chars()
            .any(|c| matches!(c, '\n' | '\r' | '\0' | '\x08'))
        {
            let msg = self.alloc(ObjectData::Str(
                "❌ fetch: URL contains illegal control characters".to_string(),
            ));
            return Ok(ExecutionFlow::Throw(msg));
        }

        // Reject suspiciously long URLs
        if url.len() > 2048 {
            let msg = self.alloc(ObjectData::Str(
                "❌ fetch: URL exceeds maximum length (2048)".to_string(),
            ));
            return Ok(ExecutionFlow::Throw(msg));
        }

        // Reject malformed methods (spaces / control chars would be header smuggling)
        if method.is_empty() || method.chars().any(|c| c.is_control() || c == ' ') {
            let msg = self.alloc(ObjectData::Str(format!(
                "❌ fetch: invalid HTTP method '{}'",
                method
            )));
            return Ok(ExecutionFlow::Throw(msg));
        }

        // DEC-M7-006 — the lockdown gate. Not catchable as an ordinary throw:
        // it is a security refusal, so it goes through `fatal_err_kind` the way
        // every other denied capability does, and `try/catch` cannot turn it
        // back into control flow.
        if !self.security.allows_fetch(&url) {
            let host = crate::permissions::host_of(&url).unwrap_or_else(|| "<unparseable>".into());
            let message = format!(
                "fetch to '{host}' is not available here — this code runs as untrusted \
                 source, and the network is closed unless the host allows it explicitly."
            );
            return self.fatal_err_kind("PermissionError", message);
        }

        // ── Perform the request ───────────────────────────────────────────────
        // Everything above is parsing and validation and is the same everywhere.
        // Only the transport differs: ureq natively, a synchronous XHR in the
        // browser. See `fetch_transport` for both.
        match self.fetch_transport_checked(&method, &url, &headers, &body_str, timeout_secs) {
            Ok(resp) => {
                // 4xx/5xx: in `full` mode build the response object anyway;
                // otherwise throw, embedding the body so the detail isn't lost.
                if resp.status >= 400 && !full {
                    let detail = if binary {
                        None
                    } else {
                        Some(String::from_utf8_lossy(&resp.body).into_owned())
                    };
                    let msg = match detail {
                        Some(b) => format!("❌ fetch: HTTP {}: {}", resp.status, b),
                        None => format!("❌ fetch: HTTP {}", resp.status),
                    };
                    let m = self.alloc(ObjectData::Str(msg));
                    return Ok(ExecutionFlow::Throw(m));
                }
                self.fetch_make_value(resp, full, binary)
            }
            Err(e) => {
                let m = self.alloc(ObjectData::Str(format!("❌ fetch: {}", e)));
                Ok(ExecutionFlow::Throw(m))
            }
        }
    }

    // Build a serez value from a response. With `full`, returns a
    // Dict<string, any> { status, ok, statusText, headers, body }; otherwise just
    // the body (string, or a byte array [int] when `binary`). Never throws on status.
    fn fetch_make_value(&mut self, resp: FetchResponse, full: bool, binary: bool) -> EvalResult {
        let status = resp.status as i64;
        let status_text = resp.status_text;
        let header_pairs: Vec<(String, String)> = if full { resp.headers } else { Vec::new() };

        let body_val: OwnedValue = if binary {
            OwnedValue::Array {
                element_type: Some("int".to_string()),
                elements: resp
                    .body
                    .into_iter()
                    .map(|b| OwnedValue::Integer(b as i64))
                    .collect(),
            }
        } else {
            OwnedValue::Str(String::from_utf8_lossy(&resp.body).into_owned())
        };

        if !full {
            // Containers (the binary byte array) must live in the global arena to
            // survive scope pops; a plain string can stay scoped like before.
            return Ok(ExecutionFlow::Value(if binary {
                self.plant_global(body_val)
            } else {
                self.plant(body_val)
            }));
        }

        let headers_dict = OwnedValue::Dict {
            key_type: "string".to_string(),
            value_type: "any".to_string(),
            entries: header_pairs
                .into_iter()
                .map(|(k, v)| (OwnedValue::Str(k), OwnedValue::Str(v)))
                .collect(),
        };
        let resp_dict = OwnedValue::Dict {
            key_type: "string".to_string(),
            value_type: "any".to_string(),
            entries: vec![
                (
                    OwnedValue::Str("status".to_string()),
                    OwnedValue::Integer(status),
                ),
                (
                    OwnedValue::Str("ok".to_string()),
                    OwnedValue::Boolean(status < 400),
                ),
                (
                    OwnedValue::Str("statusText".to_string()),
                    OwnedValue::Str(status_text),
                ),
                (OwnedValue::Str("headers".to_string()), headers_dict),
                (OwnedValue::Str("body".to_string()), body_val),
            ],
        };
        Ok(ExecutionFlow::Value(self.plant_global(resp_dict)))
    }

    // ── fetch transport ───────────────────────────────────────────────────────
    // Native: ureq. A 4xx/5xx comes back as Err(Status) there, but it is a real
    // response, so it is normalised into Ok — the caller decides whether a status
    // is an error (it depends on the `full` option).
    /// The transport, with every redirect hop checked against the allowlist.
    ///
    /// **DEC-M7-006, the half that is easy to miss.** Gating the URL the program
    /// wrote is not enough: `ureq` follows redirects itself, so
    ///
    /// ```text
    /// allowed.example  ->  302  ->  forbidden.internal
    /// ```
    ///
    /// would have reached the second host without anything asking. Under an
    /// allowlist the agent is built with `redirects(0)` and the hops are followed
    /// here, one at a time, each one validated before it is requested.
    ///
    /// **Outside lockdown this is not in the path at all.** `allows_fetch` is
    /// always true there, so the call goes straight to `fetch_transport` and the
    /// agent keeps ureq's own redirect handling — the behaviour `sz file.sz` has
    /// today, unchanged.
    fn fetch_transport_checked(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_secs: u64,
    ) -> Result<FetchResponse, String> {
        if !self.security.lockdown {
            return self.fetch_transport(method, url, headers, body, timeout_secs);
        }

        // Same ceiling ureq uses by default, so a legitimate chain that worked
        // before still works.
        const MAX_REDIRECTS: usize = 5;
        let mut current = url.to_string();
        for _ in 0..=MAX_REDIRECTS {
            // Re-checked every hop, including the first: this function must be
            // safe on its own rather than because a caller checked once.
            if !self.security.allows_fetch(&current) {
                let host =
                    crate::permissions::host_of(&current).unwrap_or_else(|| "<unparseable>".into());
                return Err(format!(
                    "redirected to '{host}', which this code is not allowed to reach"
                ));
            }
            let resp =
                self.fetch_transport_no_redirect(method, &current, headers, body, timeout_secs)?;
            if !matches!(resp.status, 301 | 302 | 303 | 307 | 308) {
                return Ok(resp);
            }
            let Some(location) = resp
                .headers
                .iter()
                .find(|(name, _)| name == "location")
                .map(|(_, value)| value.clone())
            else {
                // A redirect status with no Location is not a redirect anyone can
                // follow; hand the response back rather than inventing a target.
                return Ok(resp);
            };
            current = match crate::permissions::resolve_location(&current, &location) {
                Some(next) => next,
                None => {
                    return Err(format!(
                        "redirect to a location this runtime will not resolve: '{location}'"
                    ));
                }
            };
        }
        Err("too many redirects".to_string())
    }

    /// One hop, with ureq's redirect following switched off.
    fn fetch_transport_no_redirect(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_secs: u64,
    ) -> Result<FetchResponse, String> {
        self.fetch_transport_with(method, url, headers, body, timeout_secs, Some(0))
    }

    fn fetch_transport(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_secs: u64,
    ) -> Result<FetchResponse, String> {
        self.fetch_transport_with(method, url, headers, body, timeout_secs, None)
    }

    fn fetch_transport_with(
        &mut self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: &str,
        timeout_secs: u64,
        redirects: Option<u32>,
    ) -> Result<FetchResponse, String> {
        use std::io::Read;

        let mut builder = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(timeout_secs.min(30)))
            .timeout(std::time::Duration::from_secs(timeout_secs));
        if let Some(limit) = redirects {
            builder = builder.redirects(limit);
        }
        let agent = builder.build();

        let mut req = agent.request(method, url);
        // Default JSON content-type only when a body is sent and the user didn't set one.
        let has_ct = headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
        if !body.is_empty() && !has_ct {
            req = req.set("Content-Type", "application/json");
        }
        // Send an identifiable User-Agent unless the caller set one. Without it
        // ureq sends "ureq/x.y", which many CDNs/WAFs answer with a 503.
        let has_ua = headers
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("user-agent"));
        if !has_ua {
            req = req.set(
                "User-Agent",
                concat!("Serez-Code/", env!("CARGO_PKG_VERSION")),
            );
        }
        for (k, v) in headers {
            req = req.set(k, v);
        }

        let resp = match if body.is_empty() {
            req.call()
        } else {
            req.send_string(body)
        } {
            Ok(r) => r,
            Err(ureq::Error::Status(_, r)) => r,
            Err(e) => return Err(format!("request failed: {}", e)),
        };

        let status = resp.status();
        let status_text = resp.status_text().to_string();
        let header_pairs: Vec<(String, String)> = resp
            .headers_names()
            .iter()
            .filter_map(|n| resp.header(n).map(|v| (n.to_lowercase(), v.to_string())))
            .collect();

        let mut buf: Vec<u8> = Vec::new();
        if let Err(e) = resp.into_reader().read_to_end(&mut buf) {
            return Err(format!("failed to read response body: {}", e));
        }

        Ok(FetchResponse {
            status,
            status_text,
            headers: header_pairs,
            body: buf,
        })
    }

    // Look up a string key in dict entries (used to read fetch options).
    fn fetch_dict_get<'a>(
        entries: &'a [(OwnedValue, OwnedValue)],
        key: &str,
    ) -> Option<&'a OwnedValue> {
        entries
            .iter()
            .find(|(k, _)| matches!(k, OwnedValue::Str(s) if s.as_str() == key))
            .map(|(_, v)| v)
    }

    // ── time() — milliseconds since UNIX epoch ─────────────────────────────────
    pub(super) fn eval_builtin_time(&mut self) -> EvalResult {
        let ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis() as i64)
            .unwrap_or(0);
        Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(ms))))
    }

    // ── env(name) — read environment variable ─────────────────────────────────
    pub(super) fn eval_builtin_env(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            return self.rt_err_kind("TypeError", "env(name) requires exactly 1 argument");
        }
        let name = match self.eval_expression(&args[0]) {
            Ok(ExecutionFlow::Value(r)) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => s,
                _ => {
                    return self.rt_err_kind("TypeError", "env() argument must be a string");
                }
            },
            Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
            _ => return Err(RuntimeFailure),
        };
        let val = std::env::var(&name).unwrap_or_default();
        Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(val))))
    }

    // ── exit(code) — terminate the process ────────────────────────────────────
    pub(super) fn eval_builtin_exit(&mut self, args: &[ast::Expression]) -> EvalResult {
        let code = if args.is_empty() {
            0i32
        } else {
            match self.eval_expression(&args[0]) {
                Ok(ExecutionFlow::Value(r)) => match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(n)) => n as i32,
                    _ => {
                        return self.rt_err_kind("TypeError", "exit() argument must be an integer");
                    }
                },
                Ok(ExecutionFlow::Throw(v)) => return Ok(ExecutionFlow::Throw(v)),
                _ => return Err(RuntimeFailure),
            }
        };
        std::process::exit(code);
    }
}
