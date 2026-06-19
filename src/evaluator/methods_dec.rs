// Methods on the exact `dec` value and the `Dec` static namespace.
//
// `dec` is an exact base-10 decimal (rust_decimal): 28-29 significant digits,
// preserves scale (12.50m prints "12.50"). Rounding is explicit via round/
// setScale; the default strategy is half-even (banker's). COBOL's ROUNDED maps
// to "half-up" and MOVE truncation to "down".

use crate::ast;
use crate::region::ObjectData;
use super::EvalResult;
use rust_decimal::{Decimal, RoundingStrategy};
use rust_decimal::prelude::*;

// Map a rounding-mode name to a rust_decimal strategy. Default = half-even.
fn rounding_strategy(name: &str) -> Option<RoundingStrategy> {
    match name {
        "half-even" => Some(RoundingStrategy::MidpointNearestEven),
        "half-up"   => Some(RoundingStrategy::MidpointAwayFromZero),
        "down"      => Some(RoundingStrategy::ToZero),
        "up"        => Some(RoundingStrategy::AwayFromZero),
        "floor"     => Some(RoundingStrategy::ToNegativeInfinity),
        "ceil"      => Some(RoundingStrategy::ToPositiveInfinity),
        _           => None,
    }
}

impl super::Evaluator {
    // ── Instance methods on a dec value ───────────────────────────────────────
    pub(super) fn eval_dec_method(&mut self, d: Decimal, dot_call: &ast::DotCallExpression) -> EvalResult {
        let method = dot_call.method.as_str();

        // Evaluate every argument up front as an Integer or a string (the only
        // argument shapes dec methods take: a scale int and a mode string).
        // round / setScale / truncate: (n [, mode])
        match method {
            "round" | "setScale" | "truncate" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    eprintln!("❌ ERROR: dec.{}(n [, mode]) takes 1 or 2 arguments", method);
                    return EvalResult::Error;
                }
                let n = match self.dec_arg_int(&dot_call.arguments[0]) {
                    Ok(v) if v >= 0 && v <= 28 => v as u32,
                    Ok(_) => { eprintln!("❌ ERROR: dec.{} scale must be 0..=28", method); return EvalResult::Error; }
                    Err(e) => return e,
                };
                let strategy = if method == "truncate" {
                    RoundingStrategy::ToZero
                } else if dot_call.arguments.len() == 2 {
                    match self.dec_arg_str(&dot_call.arguments[1]) {
                        Ok(s) => match rounding_strategy(&s) {
                            Some(st) => st,
                            None => { eprintln!("❌ ERROR: unknown rounding mode '{}' (half-even|half-up|down|up|floor|ceil)", s); return EvalResult::Error; }
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
                return EvalResult::Value(self.alloc(ObjectData::Dec(out)));
            }
            _ => {}
        }

        // Zero-argument methods.
        match method {
            "scale" => EvalResult::Value(self.alloc(ObjectData::Integer(d.scale() as i64))),
            "abs" => EvalResult::Value(self.alloc(ObjectData::Dec(d.abs()))),
            "floor" => EvalResult::Value(self.alloc(ObjectData::Dec(d.floor()))),
            "ceil" => EvalResult::Value(self.alloc(ObjectData::Dec(d.ceil()))),
            "isZero" => EvalResult::Value(self.bool_ref(d.is_zero())),
            "sign" => {
                let s = if d.is_zero() { 0 } else if d.is_sign_negative() { -1 } else { 1 };
                EvalResult::Value(self.alloc(ObjectData::Integer(s)))
            }
            "toString" => EvalResult::Value(self.alloc(ObjectData::Str(d.to_string()))),
            "toInt" => match d.trunc().to_i64() {
                Some(i) => EvalResult::Value(self.alloc(ObjectData::Integer(i))),
                None => { eprintln!("❌ ERROR: dec.toInt() out of i64 range"); EvalResult::Error }
            },
            "toDecimal" => match d.to_f64() {
                Some(f) => EvalResult::Value(self.alloc(ObjectData::Decimal(f))),
                None => { eprintln!("❌ ERROR: dec.toDecimal() not representable as f64"); EvalResult::Error }
            },
            // min / max take one dec (or int) argument.
            "min" | "max" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: dec.{}(other) requires 1 argument", method);
                    return EvalResult::Error;
                }
                let other = match self.dec_arg_dec(&dot_call.arguments[0]) {
                    Ok(v) => v,
                    Err(e) => return e,
                };
                let out = if method == "min" { d.min(other) } else { d.max(other) };
                EvalResult::Value(self.alloc(ObjectData::Dec(out)))
            }
            other => {
                eprintln!("❌ ERROR: Unknown dec method '{}'", other);
                EvalResult::Error
            }
        }
    }

    // ── Static namespace: Dec.parse / fromInt / MAX / MIN / MAX_SCALE ─────────
    pub(super) fn eval_dec_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "parse" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Dec.parse(string) requires 1 argument");
                    return EvalResult::Error;
                }
                let s = match self.dec_arg_str(&dot_call.arguments[0]) {
                    Ok(s) => s,
                    Err(e) => return e,
                };
                let parsed = if s.contains('e') || s.contains('E') {
                    Decimal::from_scientific(s.trim()).ok()
                } else {
                    s.trim().parse::<Decimal>().ok()
                };
                match parsed {
                    Some(d) => EvalResult::Value(self.alloc(ObjectData::Dec(d))),
                    None => { eprintln!("❌ ERROR: Dec.parse: invalid decimal '{}'", s); EvalResult::Error }
                }
            }
            "fromInt" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: Dec.fromInt(value, scale) requires 2 integers");
                    return EvalResult::Error;
                }
                let value = match self.dec_arg_int(&dot_call.arguments[0]) { Ok(v) => v, Err(e) => return e };
                let scale = match self.dec_arg_int(&dot_call.arguments[1]) {
                    Ok(v) if v >= 0 && v <= 28 => v as u32,
                    Ok(_) => { eprintln!("❌ ERROR: Dec.fromInt scale must be 0..=28"); return EvalResult::Error; }
                    Err(e) => return e,
                };
                EvalResult::Value(self.alloc(ObjectData::Dec(Decimal::new(value, scale))))
            }
            "MAX" => EvalResult::Value(self.alloc(ObjectData::Dec(Decimal::MAX))),
            "MIN" => EvalResult::Value(self.alloc(ObjectData::Dec(Decimal::MIN))),
            "MAX_SCALE" => EvalResult::Value(self.alloc(ObjectData::Integer(28))),
            other => {
                eprintln!("❌ ERROR: Unknown Dec method '{}' (expected parse/fromInt/MAX/MIN/MAX_SCALE)", other);
                EvalResult::Error
            }
        }
    }

    // ── small argument helpers ────────────────────────────────────────────────
    fn dec_arg_int(&mut self, e: &ast::Expression) -> Result<i64, EvalResult> {
        let r = match self.eval_expression(e) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            _ => return Err(EvalResult::Error),
        };
        match self.resolve(r) {
            Some(ObjectData::Integer(n)) => Ok(*n),
            _ => { eprintln!("❌ ERROR: expected an integer argument"); Err(EvalResult::Error) }
        }
    }

    fn dec_arg_str(&mut self, e: &ast::Expression) -> Result<String, EvalResult> {
        let r = match self.eval_expression(e) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            _ => return Err(EvalResult::Error),
        };
        match self.resolve(r) {
            Some(ObjectData::Str(s)) => Ok(s.clone()),
            _ => { eprintln!("❌ ERROR: expected a string argument"); Err(EvalResult::Error) }
        }
    }

    fn dec_arg_dec(&mut self, e: &ast::Expression) -> Result<Decimal, EvalResult> {
        let r = match self.eval_expression(e) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return Err(EvalResult::Throw(v)),
            _ => return Err(EvalResult::Error),
        };
        match self.resolve(r) {
            Some(ObjectData::Dec(d)) => Ok(*d),
            Some(ObjectData::Integer(n)) => Ok(Decimal::from(*n)),
            _ => { eprintln!("❌ ERROR: expected a dec (or int) argument"); Err(EvalResult::Error) }
        }
    }
}
