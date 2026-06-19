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
            Some(ObjectData::Dec(_))      => "dec",
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
            Some(ObjectData::Ptr(_)) => "ptr",
            Some(ObjectData::Null) | None => "null",
            Some(ObjectData::EnumVariant { enum_name, .. }) => {
                let name = enum_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return EvalResult::Value(s);
            }
            Some(ObjectData::Set { .. }) => "Set",
            Some(ObjectData::Tensor { .. }) => "Tensor",
            Some(ObjectData::DateTime { .. }) => "DateTime",
            // A DateField behaves as an int under operators.
            Some(ObjectData::DateField { .. }) => "int",
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
            Some(ObjectData::Decimal(d)) => {
                if !d.is_finite() || d > i64::MAX as f64 || d < i64::MIN as f64 {
                    eprintln!("❌ ERROR: parseInt: decimal value is out of int range or not finite");
                    return EvalResult::Error;
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(d as i64)))
            }
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
                    Some(ObjectData::Integer(i)) => match i.checked_abs() {
                        Some(v) => EvalResult::Value(self.alloc(ObjectData::Integer(v))),
                        None => { eprintln!("❌ ERROR: abs() overflow (i64::MIN has no positive representation)"); EvalResult::Error }
                    },
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
                if v.is_nan() || v.is_infinite() { eprintln!("❌ ERROR: floor() argument must be a finite number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Integer(v.floor() as i64)))
            }
            "ceil" => {
                if args.len() != 1 { eprintln!("❌ ERROR: ceil() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v.is_nan() || v.is_infinite() { eprintln!("❌ ERROR: ceil() argument must be a finite number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Integer(v.ceil() as i64)))
            }
            "round" => {
                if args.len() != 1 { eprintln!("❌ ERROR: round() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v.is_nan() || v.is_infinite() { eprintln!("❌ ERROR: round() argument must be a finite number"); return EvalResult::Error; }
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
                if v < -1.0 || v > 1.0 { eprintln!("❌ ERROR: asin() argument must be in [-1, 1]"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.asin())))
            }
            "acos" => {
                if args.len() != 1 { eprintln!("❌ ERROR: acos() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v < -1.0 || v > 1.0 { eprintln!("❌ ERROR: acos() argument must be in [-1, 1]"); return EvalResult::Error; }
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
                if v.is_nan() || v.is_infinite() { eprintln!("❌ ERROR: trunc() argument must be a finite number"); return EvalResult::Error; }
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

    // ── Native: fetch ─────────────────────────────────────────────────────────

    // fetch(url, [method], [body], [options]) — general-purpose HTTP client.
    //   method/body are the string arguments after url; options is a dict:
    //     { headers: <dict>, timeout: <int secs>, full: <bool>, binary: <bool> }
    //   Default: returns the body (string), throws on HTTP status >= 400.
    //   { full: true }   → returns Dict<string, any> { status, ok, statusText, headers, body }
    //                      and does NOT throw on status.
    //   { binary: true } → body is returned as a byte array [int] instead of a string.
    pub(super) fn eval_fetch(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.is_empty() || args.len() > 4 {
            eprintln!("❌ ERROR: fetch(url, [method], [body], [options])");
            return EvalResult::Error;
        }

        // ── arg[0]: url (required, string) ────────────────────────────────────
        let url = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => s,
                _ => {
                    let msg = self.alloc(ObjectData::Str("❌ fetch: url must be a string".to_string()));
                    return EvalResult::Throw(msg);
                }
            },
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };

        // ── args[1..]: 1st string = method, 2nd string = body, dict = options ──
        let mut method: Option<String> = None;
        let mut body_str = String::new();
        let mut body_set = false;
        let mut options: Option<ObjectData> = None;
        for arg in &args[1..] {
            let r = match self.eval_expression(arg) {
                EvalResult::Value(r) => r,
                EvalResult::Throw(v) => return EvalResult::Throw(v),
                _ => return EvalResult::Error,
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
                            "❌ fetch: too many string arguments (expected method, body)".to_string()));
                        return EvalResult::Throw(msg);
                    }
                }
                Some(d @ ObjectData::Dict { .. }) => {
                    if options.is_some() {
                        let msg = self.alloc(ObjectData::Str(
                            "❌ fetch: options dict provided more than once".to_string()));
                        return EvalResult::Throw(msg);
                    }
                    options = Some(d);
                }
                _ => {
                    let msg = self.alloc(ObjectData::Str(
                        "❌ fetch: arguments after url must be strings (method/body) or a dict (options)".to_string()));
                    return EvalResult::Throw(msg);
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
            if let Some(OwnedValue::Dict { entries: hentries, .. }) = Self::fetch_dict_get(entries, "headers") {
                for (k, v) in hentries {
                    let name = match k {
                        OwnedValue::Str(s) => s.clone(),
                        _ => {
                            let msg = self.alloc(ObjectData::Str(
                                "❌ fetch: header names must be strings".to_string()));
                            return EvalResult::Throw(msg);
                        }
                    };
                    let value = v.display_str();
                    if name.chars().chain(value.chars()).any(|c| matches!(c, '\n' | '\r' | '\0')) {
                        let msg = self.alloc(ObjectData::Str(
                            format!("❌ fetch: illegal control character in header '{}'", name)));
                        return EvalResult::Throw(msg);
                    }
                    headers.push((name, value));
                }
            }
            if let Some(OwnedValue::Integer(n)) = Self::fetch_dict_get(entries, "timeout") {
                if *n > 0 { timeout_secs = *n as u64; }
            }
            if let Some(OwnedValue::Boolean(b)) = Self::fetch_dict_get(entries, "full") { full = *b; }
            if let Some(OwnedValue::Boolean(b)) = Self::fetch_dict_get(entries, "binary") { binary = *b; }
        }

        // ── Security validation ───────────────────────────────────────────────
        let lower = url.to_lowercase();
        if !lower.starts_with("http://") && !lower.starts_with("https://") {
            let msg = self.alloc(ObjectData::Str(
                format!("❌ fetch: only http:// and https:// URLs are allowed (got: {})", url)
            ));
            return EvalResult::Throw(msg);
        }

        // Reject control characters (header injection, etc.)
        if url.chars().any(|c| matches!(c, '\n' | '\r' | '\0' | '\x08')) {
            let msg = self.alloc(ObjectData::Str(
                "❌ fetch: URL contains illegal control characters".to_string()
            ));
            return EvalResult::Throw(msg);
        }

        // Reject suspiciously long URLs
        if url.len() > 2048 {
            let msg = self.alloc(ObjectData::Str(
                "❌ fetch: URL exceeds maximum length (2048)".to_string()
            ));
            return EvalResult::Throw(msg);
        }

        // Reject malformed methods (spaces / control chars would be header smuggling)
        if method.is_empty() || method.chars().any(|c| c.is_control() || c == ' ') {
            let msg = self.alloc(ObjectData::Str(
                format!("❌ fetch: invalid HTTP method '{}'", method)
            ));
            return EvalResult::Throw(msg);
        }

        // ── Build request ─────────────────────────────────────────────────────
        let agent = ureq::AgentBuilder::new()
            .timeout_connect(std::time::Duration::from_secs(timeout_secs.min(30)))
            .timeout(std::time::Duration::from_secs(timeout_secs))
            .build();

        let mut req = agent.request(&method, &url);
        // Default JSON content-type only when a body is sent and the user didn't set one.
        let has_ct = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("content-type"));
        if !body_str.is_empty() && !has_ct {
            req = req.set("Content-Type", "application/json");
        }
        // Send an identifiable User-Agent unless the caller set one. Without it
        // ureq sends "ureq/x.y", which many CDNs/WAFs answer with a 503.
        let has_ua = headers.iter().any(|(k, _)| k.eq_ignore_ascii_case("user-agent"));
        if !has_ua {
            req = req.set("User-Agent", concat!("Serez-Code/", env!("CARGO_PKG_VERSION")));
        }
        for (k, v) in &headers {
            req = req.set(k, v);
        }
        let response_result = if body_str.is_empty() {
            req.call()
        } else {
            req.send_string(&body_str)
        };

        // ── Handle response ───────────────────────────────────────────────────
        match response_result {
            Ok(resp) => self.fetch_make_value(resp, full, binary),
            // ureq returns 4xx/5xx as Err(Status). In `full` mode build the response
            // object anyway; otherwise throw, embedding the body so the error detail
            // isn't lost.
            Err(ureq::Error::Status(code, resp)) => {
                if full {
                    self.fetch_make_value(resp, true, binary)
                } else {
                    let detail = if binary { None } else { resp.into_string().ok() };
                    let msg = match detail {
                        Some(b) => format!("❌ fetch: HTTP {}: {}", code, b),
                        None => format!("❌ fetch: HTTP {}", code),
                    };
                    let m = self.alloc(ObjectData::Str(msg));
                    EvalResult::Throw(m)
                }
            }
            Err(e) => {
                let m = self.alloc(ObjectData::Str(
                    format!("❌ fetch: request failed: {}", e)
                ));
                EvalResult::Throw(m)
            }
        }
    }

    // Build a serez value from a ureq Response. With `full`, returns a
    // Dict<string, any> { status, ok, statusText, headers, body }; otherwise just
    // the body (string, or a byte array [int] when `binary`). Never throws on status.
    fn fetch_make_value(&mut self, resp: ureq::Response, full: bool, binary: bool) -> EvalResult {
        use std::io::Read;
        let status = resp.status() as i64;
        let status_text = resp.status_text().to_string();
        // Collect response headers before the body consumes `resp`.
        let header_pairs: Vec<(String, String)> = if full {
            resp.headers_names()
                .iter()
                .filter_map(|n| resp.header(n).map(|v| (n.to_lowercase(), v.to_string())))
                .collect()
        } else {
            Vec::new()
        };

        let body_val: OwnedValue = if binary {
            let mut buf: Vec<u8> = Vec::new();
            if let Err(e) = resp.into_reader().read_to_end(&mut buf) {
                let m = self.alloc(ObjectData::Str(
                    format!("❌ fetch: failed to read response body: {}", e)));
                return EvalResult::Throw(m);
            }
            OwnedValue::Array {
                element_type: Some("int".to_string()),
                elements: buf.into_iter().map(|b| OwnedValue::Integer(b as i64)).collect(),
            }
        } else {
            match resp.into_string() {
                Ok(s) => OwnedValue::Str(s),
                Err(e) => {
                    let m = self.alloc(ObjectData::Str(
                        format!("❌ fetch: failed to read response body: {}", e)));
                    return EvalResult::Throw(m);
                }
            }
        };

        if !full {
            // Containers (the binary byte array) must live in the global arena to
            // survive scope pops; a plain string can stay scoped like before.
            return EvalResult::Value(if binary {
                self.plant_global(body_val)
            } else {
                self.plant(body_val)
            });
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
                (OwnedValue::Str("status".to_string()), OwnedValue::Integer(status)),
                (OwnedValue::Str("ok".to_string()), OwnedValue::Boolean(status < 400)),
                (OwnedValue::Str("statusText".to_string()), OwnedValue::Str(status_text)),
                (OwnedValue::Str("headers".to_string()), headers_dict),
                (OwnedValue::Str("body".to_string()), body_val),
            ],
        };
        EvalResult::Value(self.plant_global(resp_dict))
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
        EvalResult::Value(self.alloc(ObjectData::Integer(ms)))
    }

    // ── env(name) — read environment variable ─────────────────────────────────
    pub(super) fn eval_builtin_env(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: env(name) requires exactly 1 argument");
            return EvalResult::Error;
        }
        let name = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => s,
                _ => { eprintln!("❌ ERROR: env() argument must be a string"); return EvalResult::Error; }
            },
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let val = std::env::var(&name).unwrap_or_default();
        EvalResult::Value(self.alloc(ObjectData::Str(val)))
    }

    // ── exit(code) — terminate the process ────────────────────────────────────
    pub(super) fn eval_builtin_exit(&mut self, args: &[ast::Expression]) -> EvalResult {
        let code = if args.is_empty() {
            0i32
        } else {
            match self.eval_expression(&args[0]) {
                EvalResult::Value(r) => match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(n)) => n as i32,
                    _ => { eprintln!("❌ ERROR: exit() argument must be an integer"); return EvalResult::Error; }
                },
                EvalResult::Throw(v) => return EvalResult::Throw(v),
                _ => return EvalResult::Error,
            }
        };
        std::process::exit(code);
    }

}
