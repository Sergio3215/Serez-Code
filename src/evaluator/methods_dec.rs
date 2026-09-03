use super::ExecutionFlow;
// Methods on the exact `dec` value and the `Dec` static namespace.
//
// `dec` is an exact base-10 decimal (rust_decimal): 28-29 significant digits,
// preserves scale (12.50m prints "12.50"). Rounding is explicit via round/
// setScale; the default strategy is half-even (banker's). COBOL's ROUNDED maps
// to "half-up" and MOVE truncation to "down".

use super::EvalResult;
use crate::ast;
use crate::region::ObjectData;
use rust_decimal::prelude::*;
use rust_decimal::{Decimal, RoundingStrategy};

// Map a rounding-mode name to a rust_decimal strategy. Default = half-even.
fn rounding_strategy(name: &str) -> Option<RoundingStrategy> {
    match name {
        "half-even" => Some(RoundingStrategy::MidpointNearestEven),
        "half-up" => Some(RoundingStrategy::MidpointAwayFromZero),
        "down" => Some(RoundingStrategy::ToZero),
        "up" => Some(RoundingStrategy::AwayFromZero),
        "floor" => Some(RoundingStrategy::ToNegativeInfinity),
        "ceil" => Some(RoundingStrategy::ToPositiveInfinity),
        _ => None,
    }
}

impl super::Evaluator {
    // ── Instance methods on a dec value ───────────────────────────────────────
    pub(super) fn eval_dec_method(
        &mut self,
        d: Decimal,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let method = dot_call.method.as_str();

        // Evaluate every argument up front as an Integer or a string (the only
        // argument shapes dec methods take: a scale int and a mode string).
        // round / setScale / truncate: (n [, mode])
        match method {
            "round" | "setScale" | "truncate" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    let given = dot_call.arguments.len();
                    return self.rt_err_kind(
                        "TypeError",
                        format!("dec.{method}(n [, mode]) takes 1 or 2 arguments, got {given}"),
                    );
                }
                let n = match self.dec_arg_int(&dot_call.arguments[0], method, "scale") {
                    Ok(v) if (0..=28).contains(&v) => v as u32,
                    Ok(out_of_range) => {
                        return self.rt_err_kind(
                            "RangeError",
                            format!("dec.{method}: scale must be 0..=28, got {out_of_range}"),
                        );
                    }
                    Err(e) => return e,
                };
                let strategy = if method == "truncate" {
                    RoundingStrategy::ToZero
                } else if dot_call.arguments.len() == 2 {
                    match self.dec_arg_str(&dot_call.arguments[1], method, "mode") {
                        Ok(s) => match rounding_strategy(&s) {
                            Some(st) => st,
                            None => {
                                return self.rt_err_kind(
                                    "RangeError",
                                    format!(
                                        "unknown rounding mode '{s}' \
                                         (half-even|half-up|down|up|floor|ceil)"
                                    ),
                                );
                            }
                        },
                        Err(e) => return e,
                    }
                } else {
                    RoundingStrategy::MidpointNearestEven // default: half-even
                };
                let mut out = d.round_dp_with_strategy(n, strategy);
                // setScale fixes the scale to exactly n (pads with zeros), as a
                // COBOL PIC V99 always carries its declared decimals (1m → 1.00).
                // round / truncate only round, leaving the natural scale.
                if method == "setScale" {
                    out.rescale(n);
                }
                return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(out))));
            }
            _ => {}
        }

        // Zero-argument methods.
        match method {
            "scale" => Ok(ExecutionFlow::Value(
                self.alloc(ObjectData::Integer(d.scale() as i64)),
            )),
            "abs" => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(d.abs())))),
            "floor" => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(d.floor())))),
            "ceil" => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(d.ceil())))),
            "isZero" => Ok(ExecutionFlow::Value(self.bool_ref(d.is_zero()))),
            "sign" => {
                let s = if d.is_zero() {
                    0
                } else if d.is_sign_negative() {
                    -1
                } else {
                    1
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(s))))
            }
            "toString" => Ok(ExecutionFlow::Value(
                self.alloc(ObjectData::Str(d.to_string())),
            )),
            "toInt" => match d.trunc().to_i64() {
                Some(i) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(i)))),
                None => self.rt_err_kind("Overflow", "dec.toInt() out of i64 range"),
            },
            "toDecimal" => match d.to_f64() {
                Some(f) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(f)))),
                None => self.rt_err_kind("Overflow", "dec.toDecimal() not representable as f64"),
            },
            // min / max take one dec (or int) argument.
            "min" | "max" => {
                if dot_call.arguments.len() != 1 {
                    let given = dot_call.arguments.len();
                    return self.rt_err_kind(
                        "TypeError",
                        format!("dec.{method}(other) requires 1 argument, got {given}"),
                    );
                }
                let other = match self.dec_arg_dec(&dot_call.arguments[0], method, "other") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let out = if method == "min" {
                    d.min(other)
                } else {
                    d.max(other)
                };
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(out))))
            }
            other => {
                let message = format!("Unknown dec method '{other}'");
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    // ── Static namespace: Dec.parse / fromInt / MAX / MIN / MAX_SCALE ─────────
    pub(super) fn eval_dec_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "parse" => {
                if dot_call.arguments.len() != 1 {
                    let given = dot_call.arguments.len();
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Dec.parse(string) requires 1 argument, got {given}"),
                    );
                }
                let s = match self.dec_arg_str(&dot_call.arguments[0], "Dec.parse", "value") {
                    Ok(s) => s,
                    Err(e) => return e,
                };
                let parsed = if s.contains('e') || s.contains('E') {
                    Decimal::from_scientific(s.trim()).ok()
                } else {
                    s.trim().parse::<Decimal>().ok()
                };
                match parsed {
                    Some(d) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(d)))),
                    None => {
                        let message = format!("Dec.parse: invalid decimal '{s}'");
                        self.rt_err_kind("RangeError", message)
                    }
                }
            }
            "fromInt" => {
                if dot_call.arguments.len() != 2 {
                    let given = dot_call.arguments.len();
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Dec.fromInt(value, scale) requires 2 integers, got {given}"),
                    );
                }
                let value = match self.dec_arg_int(&dot_call.arguments[0], "Dec.fromInt", "value") {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let scale = match self.dec_arg_int(&dot_call.arguments[1], "Dec.fromInt", "scale") {
                    Ok(v) if (0..=28).contains(&v) => v as u32,
                    Ok(out_of_range) => {
                        return self.rt_err_kind(
                            "RangeError",
                            format!("Dec.fromInt: scale must be 0..=28, got {out_of_range}"),
                        );
                    }
                    Err(e) => return e,
                };
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Dec(Decimal::new(value, scale))),
                ))
            }
            "MAX" => {
                if let Some(error) = self.reject_arguments(dot_call, "Dec") {
                    return error;
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Dec(Decimal::MAX)),
                ))
            }
            "MIN" => {
                if let Some(error) = self.reject_arguments(dot_call, "Dec") {
                    return error;
                }
                Ok(ExecutionFlow::Value(
                    self.alloc(ObjectData::Dec(Decimal::MIN)),
                ))
            }
            "MAX_SCALE" => {
                if let Some(error) = self.reject_arguments(dot_call, "Dec") {
                    return error;
                }
                Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(28))))
            }
            other => {
                let message = format!(
                    "Unknown Dec method '{other}' (expected parse/fromInt/MAX/MIN/MAX_SCALE)"
                );
                self.rt_err_kind("ReferenceError", message)
            }
        }
    }

    // ── small argument helpers ────────────────────────────────────────────────
    fn dec_arg_int(
        &mut self,
        e: &ast::Expression,
        context: &str,
        parameter: &str,
    ) -> Result<i64, EvalResult> {
        let r = match self.eval_expression(e) {
            Ok(ExecutionFlow::Value(r)) => r,
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) => Ok(*n),
            _ => Err(self.rt_err_kind(
                "TypeError",
                format!("{context}: {parameter} must be an int"),
            )),
        }
    }

    fn dec_arg_str(
        &mut self,
        e: &ast::Expression,
        context: &str,
        parameter: &str,
    ) -> Result<String, EvalResult> {
        let r = match self.eval_expression(e) {
            Ok(ExecutionFlow::Value(r)) => r,
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Str(s)) => Ok(s.clone()),
            _ => Err(self.rt_err_kind(
                "TypeError",
                format!("{context}: {parameter} must be a string"),
            )),
        }
    }

    fn dec_arg_dec(
        &mut self,
        e: &ast::Expression,
        context: &str,
        parameter: &str,
    ) -> Result<Decimal, EvalResult> {
        let r = match self.eval_expression(e) {
            Ok(ExecutionFlow::Value(r)) => r,
            other => return Err(other),
        };
        match self.resolve(r) {
            Some(ObjectData::Dec(d)) => Ok(*d),
            Some(ObjectData::Integer(n)) => Ok(Decimal::from(*n)),
            _ => Err(self.rt_err_kind(
                "TypeError",
                format!("{context}: {parameter} must be a dec or an int"),
            )),
        }
    }
}
