#![allow(unused_imports)]
use super::ExecutionFlow;
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

// How an instance renders when concatenated with a string (`"x" + obj`). The
// built-in Error object (bound in `catch (e)`) renders as its `message`, so the
// common `"error: " + e` pattern keeps working; any other instance renders as
// `ClassName{ field: value, ... }`.
fn instance_concat_str(class_name: &str, fields: &[(String, OwnedValue)]) -> String {
    if class_name == "Error" {
        if let Some((_, v)) = fields.iter().find(|(k, _)| k == "message") {
            return v.display_str();
        }
    }
    let parts: Vec<String> = fields
        .iter()
        .map(|(k, v)| format!("{}: {}", k, v.display_str()))
        .collect();
    format!("{}{{ {} }}", class_name, parts.join(", "))
}

impl super::Evaluator {
    pub(super) fn eval_prefix(
        &mut self,
        op: &str,
        right_ref: ObjectRef,
        right: ObjectData,
    ) -> EvalResult {
        match op {
            "-" => match right {
                ObjectData::Integer(i) => match i.checked_neg() {
                    Some(v) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(v)))),
                    None => self.rt_err_kind(
                        "Overflow",
                        "Integer overflow in negation (i64::MIN has no positive counterpart)",
                    ),
                },
                ObjectData::Decimal(d) => {
                    Ok(ExecutionFlow::Value(self.alloc(ObjectData::Decimal(-d))))
                }
                ObjectData::Dec(d) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(-d)))),
                ObjectData::Instance { ref class_name, .. } => {
                    let cn = class_name.clone();
                    if self.find_method(&cn, "op_neg").is_some() {
                        self.call_op_method(right_ref, &cn, "op_neg", vec![], 0, 0)
                    } else {
                        self.rt_err_kind(
                            "TypeError",
                            "Prefix '-' not supported for this type (define op_neg to enable it)",
                        )
                    }
                }
                _ => self.rt_err_kind("TypeError", "Prefix '-' not supported for this type"),
            },
            // `!` niega la regla ÚNICA de truthiness (`is_truthy`), la misma que
            // usan `&&`, `||`, el ternario y las guardas de `match`. Antes exigía
            // un booleano, lo que dejaba el idiom partido por la mitad: desde 9.14
            // `items && <Fila/>` compila, pero `!items` moría con
            // "Prefix '!' only applies to booleans" y había que escribir
            // `items.length() == 0`. Con booleanos el resultado es idéntico al
            // anterior, así que sólo se agregan casos que antes eran un error.
            //
            // Una instancia que define `op_not` sigue ganando: la sobrecarga es
            // una decisión explícita del autor de la clase y tiene prioridad sobre
            // la regla general (donde una instancia es truthy → `!inst` es false).
            "!" => match right {
                ObjectData::Boolean(b) => Ok(ExecutionFlow::Value(self.bool_ref(!b))),
                ObjectData::Instance { ref class_name, .. }
                    if self.find_method(class_name, "op_not").is_some() =>
                {
                    let cn = class_name.clone();
                    self.call_op_method(right_ref, &cn, "op_not", vec![], 0, 0)
                }
                ref other => {
                    let t = self.is_truthy(other);
                    Ok(ExecutionFlow::Value(self.bool_ref(!t)))
                }
            },
            "~" => match right {
                ObjectData::Integer(i) => {
                    Ok(ExecutionFlow::Value(self.alloc(ObjectData::Integer(!i))))
                }
                _ => self.rt_err_kind("TypeError", "Prefix '~' only applies to integers"),
            },
            _ => self.rt_err_kind("TypeError", format!("Unknown prefix operator: {op}")),
        }
    }

    // Exact base-10 arithmetic for `dec`. Comparisons are by value (scale
    // ignored: 1.50m == 1.5m). Arithmetic is checked (overflow → ❌, like int);
    // `/` rounds to 28 significant digits half-even (rust_decimal default);
    // `**` requires a non-negative integer exponent.
    pub(super) fn dec_binop(
        &mut self,
        op: &str,
        l: rust_decimal::Decimal,
        r: rust_decimal::Decimal,
        line: usize,
        column: usize,
    ) -> EvalResult {
        use rust_decimal::prelude::*;
        match op {
            "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
            ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
            "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
            ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
            "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
            "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
            _ => {}
        }
        let result = match op {
            "+" => l.checked_add(r),
            "-" => l.checked_sub(r),
            "*" => l.checked_mul(r),
            "%" => {
                if r.is_zero() {
                    return self.rt_err_kind(
                        "DivisionByZero",
                        format!("Decimal modulo by zero - [{line}:{column}]"),
                    );
                }
                l.checked_rem(r)
            }
            "/" => {
                if r.is_zero() {
                    return self.rt_err_kind(
                        "DivisionByZero",
                        format!("Decimal division by zero - [{line}:{column}]"),
                    );
                }
                l.checked_div(r)
            }
            "**" => {
                if r.is_sign_negative() || r.fract() != rust_decimal::Decimal::ZERO {
                    return self.rt_err_kind(
                        "TypeError",
                        format!(
                            "'**' on dec requires a non-negative integer exponent - [{line}:{column}]"
                        ),
                    );
                }
                let exp = match r.to_u64() {
                    Some(e) => e,
                    None => {
                        return self.rt_err_kind(
                            "Overflow",
                            format!("dec exponent too large - [{line}:{column}]"),
                        );
                    }
                };
                let mut acc = rust_decimal::Decimal::ONE;
                let mut overflow = false;
                for _ in 0..exp {
                    match acc.checked_mul(l) {
                        Some(v) => acc = v,
                        None => {
                            overflow = true;
                            break;
                        }
                    }
                }
                if overflow { None } else { Some(acc) }
            }
            _ => {
                return self.rt_err_kind(
                    "TypeError",
                    format!("Operator '{op}' not supported for dec - [{line}:{column}]"),
                );
            }
        };
        match result {
            Some(v) => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Dec(v)))),
            None => self.rt_err_kind("Overflow", format!("Decimal overflow - [{line}:{column}]")),
        }
    }

    pub(super) fn eval_infix(
        &mut self,
        op: &str,
        left: ObjectData,
        right: ObjectData,
        line: usize,
        column: usize,
    ) -> EvalResult {
        // DateTime ordering/equality: compare two DateTimes by their instant.
        // Arithmetic between dates is intentionally not supported (use fields).
        if let (
            ObjectData::DateTime { epoch_ms: a, .. },
            ObjectData::DateTime { epoch_ms: b, .. },
        ) = (&left, &right)
        {
            let (a, b) = (*a, *b);
            match op {
                "<" => return Ok(ExecutionFlow::Value(self.bool_ref(a < b))),
                ">" => return Ok(ExecutionFlow::Value(self.bool_ref(a > b))),
                "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(a <= b))),
                ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(a >= b))),
                "==" => return Ok(ExecutionFlow::Value(self.bool_ref(a == b))),
                "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(a != b))),
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!(
                            "Operator '{op}' cannot be applied to DateTime - [{line}:{column}]"
                        ),
                    );
                }
            }
        }
        // A DateField acts as its integer value in every operator.
        let left = match left {
            ObjectData::DateField { value, .. } => ObjectData::Integer(value),
            other => other,
        };
        let right = match right {
            ObjectData::DateField { value, .. } => ObjectData::Integer(value),
            other => other,
        };

        // Null equality: any value can be compared to null with == / !=
        if matches!(left, ObjectData::Null) || matches!(right, ObjectData::Null) {
            // Allow string + null and null + string concatenation
            if op == "+" {
                let s = match (&left, &right) {
                    (ObjectData::Str(s), ObjectData::Null) => format!("{}null", s),
                    (ObjectData::Null, ObjectData::Str(s)) => format!("null{}", s),
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            format!("Operator '+' cannot be applied to null - [{line}:{column}]"),
                        );
                    }
                };
                return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(s))));
            }
            return match op {
                "==" => {
                    let eq = matches!(left, ObjectData::Null) && matches!(right, ObjectData::Null);
                    Ok(ExecutionFlow::Value(self.bool_ref(eq)))
                }
                "!=" => {
                    let eq = matches!(left, ObjectData::Null) && matches!(right, ObjectData::Null);
                    Ok(ExecutionFlow::Value(self.bool_ref(!eq)))
                }
                _ => self.rt_err_kind(
                    "TypeError",
                    format!("Operator '{op}' cannot be applied to null - [{line}:{column}]"),
                ),
            };
        }
        let left_type = left.type_name().to_string();
        let right_type = right.type_name().to_string();
        match (left, right) {
            (ObjectData::Integer(l), ObjectData::Integer(r)) => {
                match op {
                    "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
                    ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
                    "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
                    ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
                    "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                    "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                    _ => {}
                }
                let result = match op {
                    "+" => match l.checked_add(r) {
                        Some(v) => v,
                        None => return self.rt_err_kind("Overflow", "Integer overflow"),
                    },
                    "-" => match l.checked_sub(r) {
                        Some(v) => v,
                        None => return self.rt_err_kind("Overflow", "Integer overflow"),
                    },
                    "*" => match l.checked_mul(r) {
                        Some(v) => v,
                        None => return self.rt_err_kind("Overflow", "Integer overflow"),
                    },
                    "/" => {
                        if r == 0 {
                            return self.rt_err_kind("DivisionByZero", "Division by zero");
                        }
                        match l.checked_div(r) {
                            Some(v) => v,
                            None => return self.rt_err_kind("Overflow", "Integer overflow"),
                        }
                    }
                    "%" => {
                        if r == 0 {
                            return self.rt_err_kind("DivisionByZero", "Modulus by zero");
                        }
                        match l.checked_rem(r) {
                            Some(v) => v,
                            None => {
                                return self.rt_err_kind(
                                    "Overflow",
                                    "Modulo overflow (i64::MIN % -1 is undefined)",
                                );
                            }
                        }
                    }
                    "**" => {
                        if r < 0 {
                            return Ok(ExecutionFlow::Value(
                                self.alloc(ObjectData::Decimal((l as f64).powf(r as f64))),
                            ));
                        } else if r > u32::MAX as i64 {
                            match l {
                                0 => 0,
                                1 => 1,
                                -1 => {
                                    if r % 2 == 0 {
                                        1
                                    } else {
                                        -1
                                    }
                                }
                                _ => {
                                    return self.rt_err_kind(
                                        "Overflow",
                                        "Integer overflow in exponentiation",
                                    );
                                }
                            }
                        } else {
                            match l.checked_pow(r as u32) {
                                Some(v) => v,
                                None => {
                                    return self.rt_err_kind(
                                        "Overflow",
                                        "Integer overflow in exponentiation",
                                    );
                                }
                            }
                        }
                    }
                    "&" => l & r,
                    "|" => l | r,
                    "^" => l ^ r,
                    "<<" => {
                        if r < 0 {
                            return self.rt_err_kind(
                                "TypeError",
                                format!("Left shift by negative amount ({r})"),
                            );
                        }
                        if r >= 64 {
                            return self.rt_err_kind(
                                "TypeError",
                                format!("Left shift by {r} is >= 64 bits"),
                            );
                        }
                        l << r
                    }
                    ">>" => {
                        if r < 0 {
                            return self.rt_err_kind(
                                "TypeError",
                                format!("Right shift by negative amount ({r})"),
                            );
                        }
                        if r >= 64 {
                            return self.rt_err_kind(
                                "TypeError",
                                format!("Right shift by {r} is >= 64 bits"),
                            );
                        }
                        l >> r
                    }
                    _ => {
                        return self
                            .rt_err_kind("TypeError", format!("Unknown operator: {op}"));
                    }
                };
                Ok(ExecutionFlow::Value(self.int_ref(result)))
            }
            // Exact base-10 `dec`. `int` mixes in (it is exact); f64 `decimal`
            // is NEVER mixed implicitly — that would re-contaminate exactness.
            (ObjectData::Dec(l), ObjectData::Dec(r)) => self.dec_binop(op, l, r, line, column),
            (ObjectData::Dec(l), ObjectData::Integer(r)) => {
                self.dec_binop(op, l, rust_decimal::Decimal::from(r), line, column)
            }
            (ObjectData::Integer(l), ObjectData::Dec(r)) => {
                self.dec_binop(op, rust_decimal::Decimal::from(l), r, line, column)
            }
            (ObjectData::Dec(_), ObjectData::Decimal(_))
            | (ObjectData::Decimal(_), ObjectData::Dec(_)) => {
                self.rt_err_kind(
                    "TypeError",
                    format!(
                        "cannot mix 'dec' (exact) and 'decimal' (f64) with '{op}' — convert explicitly (d.toDecimal() / Dec.parse) - [{line}:{column}]"
                    ),
                )
            }
            (ObjectData::Str(s), ObjectData::Dec(d)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", s, d))))),
                _ => self.rt_err_kind(
                    "TypeError",
                    format!("Operator '{op}' not supported between String and dec"),
                ),
            },
            (ObjectData::Dec(d), ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", d, s))))),
                _ => self.rt_err_kind(
                    "TypeError",
                    format!("Operator '{op}' not supported between dec and String"),
                ),
            },
            // Decimal arithmetic (decimal op decimal, int op decimal, decimal op int)
            (ObjectData::Decimal(l), ObjectData::Decimal(r)) => {
                match op {
                    "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
                    ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
                    "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
                    ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
                    "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                    "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Division by zero");
                        }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Modulus by zero");
                        }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => {
                        return self
                            .rt_err_kind("TypeError", format!("Unknown operator: {op}"));
                    }
                };
                Ok(ExecutionFlow::Value(self.alloc(result)))
            }
            (ObjectData::Integer(l), ObjectData::Decimal(r)) => {
                let l = l as f64;
                match op {
                    "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
                    ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
                    "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
                    ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
                    "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                    "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Division by zero");
                        }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Modulus by zero");
                        }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => {
                        return self
                            .rt_err_kind("TypeError", format!("Operator '{op}' not supported here"));
                    }
                };
                Ok(ExecutionFlow::Value(self.alloc(result)))
            }
            (ObjectData::Decimal(l), ObjectData::Integer(r)) => {
                let r = r as f64;
                match op {
                    "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
                    ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
                    "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
                    ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
                    "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                    "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Division by zero");
                        }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 {
                            return self.rt_err_kind("DivisionByZero", "Modulus by zero");
                        }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => {
                        return self
                            .rt_err_kind("TypeError", format!("Operator '{op}' not supported here"));
                    }
                };
                Ok(ExecutionFlow::Value(self.alloc(result)))
            }

            (ObjectData::Str(l), ObjectData::Str(r)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                "<" => return Ok(ExecutionFlow::Value(self.bool_ref(l < r))),
                ">" => return Ok(ExecutionFlow::Value(self.bool_ref(l > r))),
                "<=" => return Ok(ExecutionFlow::Value(self.bool_ref(l <= r))),
                ">=" => return Ok(ExecutionFlow::Value(self.bool_ref(l >= r))),
                "+" => return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(l + &r)))),
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between strings"),
                    );
                }
            },
            (ObjectData::Str(s), ObjectData::Integer(n)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", s, n)))));
                }
                "*" => {
                    if n < 0 {
                        return self.rt_err_kind(
                            "TypeError",
                            "Cannot repeat a string with a negative n",
                        );
                    }
                    if n > 10_000_000 {
                        return self.fatal_err_kind(
                            "ResourceError",
                            format!(
                                "String repeat count {n} exceeds maximum (10,000,000)"
                            ),
                        );
                    }
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(s.repeat(n as usize)))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between String and Integer"),
                    );
                }
            },
            (ObjectData::Integer(n), ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", n, s)))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between Integer and String"),
                    );
                }
            },
            (ObjectData::Str(s), ObjectData::Decimal(d)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                        "{}{}",
                        s,
                        format_decimal(d)
                    )))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between String and Decimal"),
                    );
                }
            },
            (ObjectData::Decimal(d), ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                        "{}{}",
                        format_decimal(d),
                        s
                    )))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between Decimal and String"),
                    );
                }
            },
            (ObjectData::Str(s), ObjectData::Boolean(b)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", s, b)))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between String and Boolean"),
                    );
                }
            },
            (ObjectData::Boolean(b), ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}{}", b, s)))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between Boolean and String"),
                    );
                }
            },
            (ObjectData::Str(s), ObjectData::Null) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("{}null", s))))),
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between String and Null"),
                    );
                }
            },
            (ObjectData::Null, ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!("null{}", s))))),
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between Null and String"),
                    );
                }
            },
            // String concatenation with a DateTime renders its ISO 8601 form,
            // matching how int/decimal/bool concatenate.
            (ObjectData::Str(s), ObjectData::DateTime { epoch_ms, utc }) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                        "{}{}",
                        s,
                        crate::region::format_datetime(epoch_ms, utc)
                    )))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between String and DateTime"),
                    );
                }
            },
            (ObjectData::DateTime { epoch_ms, utc }, ObjectData::Str(s)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.false_ref)),
                "!=" => return Ok(ExecutionFlow::Value(self.true_ref)),
                "+" => {
                    return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                        "{}{}",
                        crate::region::format_datetime(epoch_ms, utc),
                        s
                    )))));
                }
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!("Operator '{op}' not supported between DateTime and String"),
                    );
                }
            },
            (ObjectData::Boolean(l), ObjectData::Boolean(r)) => match op {
                "==" => return Ok(ExecutionFlow::Value(self.bool_ref(l == r))),
                "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(l != r))),
                _ => {
                    return self.rt_err_kind(
                        "TypeError",
                        format!(
                            "Operator '{op}' not supported between booleans (use && / ||)"
                        ),
                    );
                }
            },
            // ── EnumVariant equality ─────────────────────────────────────────
            (
                ObjectData::EnumVariant {
                    enum_name: en1,
                    variant: v1,
                },
                ObjectData::EnumVariant {
                    enum_name: en2,
                    variant: v2,
                },
            ) => {
                let eq = en1 == en2 && v1 == v2;
                match op {
                    "==" => return Ok(ExecutionFlow::Value(self.bool_ref(eq))),
                    "!=" => return Ok(ExecutionFlow::Value(self.bool_ref(!eq))),
                    _ => {
                        return self.rt_err_kind(
                            "TypeError",
                            format!("Operator '{op}' not supported between enum variants"),
                        );
                    }
                }
            }

            (left, right) => {
                // ── String + instance: use op_str (consistent with interpolation
                // and array display, B-57/B-58). Checked before op_add so
                // `money + "x"` formats instead of calling op_add with a string.
                if op == "+" {
                    let left_opstr = if let ObjectData::Instance { ref class_name, .. } = left {
                        self.find_method(class_name, "op_str")
                            .map(|_| class_name.clone())
                    } else {
                        None
                    };
                    let right_opstr = if let ObjectData::Instance { ref class_name, .. } = right {
                        self.find_method(class_name, "op_str")
                            .map(|_| class_name.clone())
                    } else {
                        None
                    };

                    if let (ObjectData::Str(s), Some(cn)) = (&left, &right_opstr) {
                        let prefix = s.clone();
                        let cn = cn.clone();
                        let inst_ref = self.alloc(right);
                        return match self.call_op_method(
                            inst_ref,
                            &cn,
                            "op_str",
                            vec![],
                            line,
                            column,
                        ) {
                            Ok(ExecutionFlow::Value(r)) => {
                                let rs = self.display(r);
                                Ok(ExecutionFlow::Value(
                                    self.alloc(ObjectData::Str(format!("{}{}", prefix, rs))),
                                ))
                            }
                            other => other,
                        };
                    }
                    if let (Some(cn), ObjectData::Str(s)) = (&left_opstr, &right) {
                        let suffix = s.clone();
                        let cn = cn.clone();
                        let inst_ref = self.alloc(left);
                        return match self.call_op_method(
                            inst_ref,
                            &cn,
                            "op_str",
                            vec![],
                            line,
                            column,
                        ) {
                            Ok(ExecutionFlow::Value(r)) => {
                                let ls = self.display(r);
                                Ok(ExecutionFlow::Value(
                                    self.alloc(ObjectData::Str(format!("{}{}", ls, suffix))),
                                ))
                            }
                            other => other,
                        };
                    }
                }
                // ── Operator overloading ─────────────────────────────────────
                // Check BEFORE the equality short-circuit so op_eq/op_ne get a chance.
                let method_name = operator_to_method_name(op);
                let maybe_class = if !method_name.is_empty() {
                    if let ObjectData::Instance { ref class_name, .. } = left {
                        let has = self.find_method(class_name, method_name).is_some();
                        if has { Some(class_name.clone()) } else { None }
                    } else {
                        None
                    }
                } else {
                    None
                };

                if let Some(class_name) = maybe_class {
                    let inst_ref = self.alloc(left);
                    let arg_ref = self.alloc(right);
                    let arg_owned = self.extract(arg_ref);
                    return self.call_op_method(
                        inst_ref,
                        &class_name,
                        method_name,
                        vec![arg_owned],
                        line,
                        column,
                    );
                }

                // String + instance with NO op_str/op_add overload: render the
                // instance (built-in Error → its message), so `catch (e) { "x" + e }`
                // works now that runtime errors bind a structured Error object.
                if op == "+" {
                    if let (ObjectData::Str(s), ObjectData::Instance { class_name, fields }) =
                        (&left, &right)
                    {
                        return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                            "{}{}",
                            s,
                            instance_concat_str(class_name, fields)
                        )))));
                    }
                    if let (ObjectData::Instance { class_name, fields }, ObjectData::Str(s)) =
                        (&left, &right)
                    {
                        return Ok(ExecutionFlow::Value(self.alloc(ObjectData::Str(format!(
                            "{}{}",
                            instance_concat_str(class_name, fields),
                            s
                        )))));
                    }
                }

                // Cross-type equality: different types are never equal
                if op == "==" {
                    return Ok(ExecutionFlow::Value(self.false_ref));
                }
                if op == "!=" {
                    return Ok(ExecutionFlow::Value(self.true_ref));
                }
                self.rt_err_kind(
                    "TypeError",
                    format!(
                        "Type mismatch — operator '{op}' cannot be applied between '{left_type}' and '{right_type}' - [{line}:{column}]"
                    ),
                )
            }
        }
    }
}
