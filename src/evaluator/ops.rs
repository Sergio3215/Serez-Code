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
    pub(super) fn eval_prefix(&mut self, op: &str, right_ref: ObjectRef, right: ObjectData) -> EvalResult {
        match op {
            "-" => match right {
                ObjectData::Integer(i) => match i.checked_neg() {
                    Some(v) => EvalResult::Value(self.alloc(ObjectData::Integer(v))),
                    None => {
                        eprintln!("❌ ERROR: Integer overflow in negation (i64::MIN has no positive counterpart)");
                        EvalResult::Error
                    }
                },
                ObjectData::Decimal(d) => EvalResult::Value(self.alloc(ObjectData::Decimal(-d))),
                ObjectData::Dec(d) => EvalResult::Value(self.alloc(ObjectData::Dec(-d))),
                ObjectData::Instance { ref class_name, .. } => {
                    let cn = class_name.clone();
                    if self.find_method(&cn, "op_neg").is_some() {
                        self.call_op_method(right_ref, &cn, "op_neg", vec![], 0, 0)
                    } else {
                        eprintln!("❌ ERROR: Prefix '-' not supported for this type (define op_neg to enable it)");
                        EvalResult::Error
                    }
                }
                _ => {
                    eprintln!("❌ ERROR: Prefix '-' not supported for this type");
                    EvalResult::Error
                }
            },
            "!" => match right {
                ObjectData::Boolean(b) => EvalResult::Value(self.bool_ref(!b)),
                ObjectData::Instance { ref class_name, .. } => {
                    let cn = class_name.clone();
                    if self.find_method(&cn, "op_not").is_some() {
                        self.call_op_method(right_ref, &cn, "op_not", vec![], 0, 0)
                    } else {
                        eprintln!("❌ ERROR: Prefix '!' only applies to booleans (define op_not to enable it on instances)");
                        EvalResult::Error
                    }
                }
                _ => {
                    eprintln!("❌ ERROR: Prefix '!' only applies to booleans");
                    EvalResult::Error
                }
            },
            "~" => match right {
                ObjectData::Integer(i) => EvalResult::Value(self.alloc(ObjectData::Integer(!i))),
                _ => {
                    eprintln!("❌ ERROR: Prefix '~' only applies to integers");
                    EvalResult::Error
                }
            },
            _ => EvalResult::Error,
        }
    }

    // Exact base-10 arithmetic for `dec`. Comparisons are by value (scale
    // ignored: 1.50m == 1.5m). Arithmetic is checked (overflow → ❌, like int);
    // `/` rounds to 28 significant digits half-even (rust_decimal default);
    // `**` requires a non-negative integer exponent.
    pub(super) fn dec_binop(&mut self, op: &str, l: rust_decimal::Decimal, r: rust_decimal::Decimal, line: usize, column: usize) -> EvalResult {
        use rust_decimal::prelude::*;
        match op {
            "<"  => return EvalResult::Value(self.bool_ref(l < r)),
            ">"  => return EvalResult::Value(self.bool_ref(l > r)),
            "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
            ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
            "==" => return EvalResult::Value(self.bool_ref(l == r)),
            "!=" => return EvalResult::Value(self.bool_ref(l != r)),
            _ => {}
        }
        let result = match op {
            "+" => l.checked_add(r),
            "-" => l.checked_sub(r),
            "*" => l.checked_mul(r),
            "%" => {
                if r.is_zero() {
                    eprintln!("❌ ERROR: Decimal modulo by zero - [{}:{}]", line, column);
                    return EvalResult::Error;
                }
                l.checked_rem(r)
            }
            "/" => {
                if r.is_zero() {
                    eprintln!("❌ ERROR: Decimal division by zero - [{}:{}]", line, column);
                    return EvalResult::Error;
                }
                l.checked_div(r)
            }
            "**" => {
                if r.is_sign_negative() || r.fract() != rust_decimal::Decimal::ZERO {
                    eprintln!("❌ ERROR: '**' on dec requires a non-negative integer exponent - [{}:{}]", line, column);
                    return EvalResult::Error;
                }
                let exp = match r.to_u64() {
                    Some(e) => e,
                    None => { eprintln!("❌ ERROR: dec exponent too large - [{}:{}]", line, column); return EvalResult::Error; }
                };
                let mut acc = rust_decimal::Decimal::ONE;
                let mut overflow = false;
                for _ in 0..exp {
                    match acc.checked_mul(l) { Some(v) => acc = v, None => { overflow = true; break; } }
                }
                if overflow { None } else { Some(acc) }
            }
            _ => { eprintln!("❌ ERROR: Operator '{}' not supported for dec - [{}:{}]", op, line, column); return EvalResult::Error; }
        };
        match result {
            Some(v) => EvalResult::Value(self.alloc(ObjectData::Dec(v))),
            None => { eprintln!("❌ ERROR: Decimal overflow - [{}:{}]", line, column); EvalResult::Error }
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
        if let (ObjectData::DateTime { epoch_ms: a, .. }, ObjectData::DateTime { epoch_ms: b, .. }) = (&left, &right) {
            let (a, b) = (*a, *b);
            match op {
                "<"  => return EvalResult::Value(self.bool_ref(a < b)),
                ">"  => return EvalResult::Value(self.bool_ref(a > b)),
                "<=" => return EvalResult::Value(self.bool_ref(a <= b)),
                ">=" => return EvalResult::Value(self.bool_ref(a >= b)),
                "==" => return EvalResult::Value(self.bool_ref(a == b)),
                "!=" => return EvalResult::Value(self.bool_ref(a != b)),
                _ => {
                    eprintln!("❌ ERROR: Operator '{}' cannot be applied to DateTime - [{}:{}]", op, line, column);
                    return EvalResult::Error;
                }
            }
        }
        // A DateField acts as its integer value in every operator.
        let left = match left { ObjectData::DateField { value, .. } => ObjectData::Integer(value), other => other };
        let right = match right { ObjectData::DateField { value, .. } => ObjectData::Integer(value), other => other };

        // Null equality: any value can be compared to null with == / !=
        if matches!(left, ObjectData::Null) || matches!(right, ObjectData::Null) {
            // Allow string + null and null + string concatenation
            if op == "+" {
                let s = match (&left, &right) {
                    (ObjectData::Str(s), ObjectData::Null) => format!("{}null", s),
                    (ObjectData::Null, ObjectData::Str(s)) => format!("null{}", s),
                    _ => {
                        eprintln!(
                            "❌ ERROR: Operator '+' cannot be applied to null - [{}:{}]",
                            line, column
                        );
                        return EvalResult::Error;
                    }
                };
                return EvalResult::Value(self.alloc(ObjectData::Str(s)));
            }
            return match op {
                "==" => {
                    let eq = matches!(left, ObjectData::Null) && matches!(right, ObjectData::Null);
                    EvalResult::Value(self.bool_ref(eq))
                }
                "!=" => {
                    let eq = matches!(left, ObjectData::Null) && matches!(right, ObjectData::Null);
                    EvalResult::Value(self.bool_ref(!eq))
                }
                _ => {
                    eprintln!(
                        "❌ ERROR: Operator '{}' cannot be applied to null - [{}:{}]",
                        op, line, column
                    );
                    EvalResult::Error
                }
            };
        }
        let left_type = left.type_name().to_string();
        let right_type = right.type_name().to_string();
        match (left, right) {
            (ObjectData::Integer(l), ObjectData::Integer(r)) => {
                match op {
                    "<"  => return EvalResult::Value(self.bool_ref(l < r)),
                    ">"  => return EvalResult::Value(self.bool_ref(l > r)),
                    "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
                    ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
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
                        if r == 0 { return self.rt_err_kind("DivisionByZero", "Division by zero"); }
                        match l.checked_div(r) {
                            Some(v) => v,
                            None => return self.rt_err_kind("Overflow", "Integer overflow"),
                        }
                    }
                    "%" => {
                        if r == 0 { return self.rt_err_kind("DivisionByZero", "Modulus by zero"); }
                        match l.checked_rem(r) {
                            Some(v) => v,
                            None => return self.rt_err_kind("Overflow", "Modulo overflow (i64::MIN % -1 is undefined)"),
                        }
                    }
                    "**" => {
                        if r < 0 {
                            return EvalResult::Value(self.alloc(ObjectData::Decimal((l as f64).powf(r as f64))));
                        } else if r > u32::MAX as i64 {
                            match l {
                                0 => 0,
                                1 => 1,
                                -1 => if r % 2 == 0 { 1 } else { -1 },
                                _ => { eprintln!("❌ ERROR: Integer overflow in exponentiation"); return EvalResult::Error; }
                            }
                        } else {
                            match l.checked_pow(r as u32) {
                                Some(v) => v,
                                None => { eprintln!("❌ ERROR: Integer overflow in exponentiation"); return EvalResult::Error; }
                            }
                        }
                    }
                    "&"  => l & r,
                    "|"  => l | r,
                    "^"  => l ^ r,
                    "<<" => {
                        if r < 0 { eprintln!("❌ ERROR: Left shift by negative amount ({})", r); return EvalResult::Error; }
                        if r >= 64 { eprintln!("❌ ERROR: Left shift by {} is >= 64 bits", r); return EvalResult::Error; }
                        l << r
                    }
                    ">>" => {
                        if r < 0 { eprintln!("❌ ERROR: Right shift by negative amount ({})", r); return EvalResult::Error; }
                        if r >= 64 { eprintln!("❌ ERROR: Right shift by {} is >= 64 bits", r); return EvalResult::Error; }
                        l >> r
                    }
                    _ => { eprintln!("❌ ERROR: Unknown operator: {}", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.int_ref(result))
            }
            // Exact base-10 `dec`. `int` mixes in (it is exact); f64 `decimal`
            // is NEVER mixed implicitly — that would re-contaminate exactness.
            (ObjectData::Dec(l), ObjectData::Dec(r)) => self.dec_binop(op, l, r, line, column),
            (ObjectData::Dec(l), ObjectData::Integer(r)) => self.dec_binop(op, l, rust_decimal::Decimal::from(r), line, column),
            (ObjectData::Integer(l), ObjectData::Dec(r)) => self.dec_binop(op, rust_decimal::Decimal::from(l), r, line, column),
            (ObjectData::Dec(_), ObjectData::Decimal(_)) | (ObjectData::Decimal(_), ObjectData::Dec(_)) => {
                eprintln!("❌ ERROR: cannot mix 'dec' (exact) and 'decimal' (f64) with '{}' — convert explicitly (d.toDecimal() / Dec.parse) - [{}:{}]", op, line, column);
                EvalResult::Error
            }
            (ObjectData::Str(s), ObjectData::Dec(d)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", s, d)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and dec", op); EvalResult::Error }
                }
            }
            (ObjectData::Dec(d), ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", d, s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between dec and String", op); EvalResult::Error }
                }
            }
            // Decimal arithmetic (decimal op decimal, int op decimal, decimal op int)
            (ObjectData::Decimal(l), ObjectData::Decimal(r)) => {
                match op {
                    "<"  => return EvalResult::Value(self.bool_ref(l < r)),
                    ">"  => return EvalResult::Value(self.bool_ref(l > r)),
                    "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
                    ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Division by zero"); }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Modulus by zero"); }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => { eprintln!("❌ ERROR: Unknown operator: {}", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Integer(l), ObjectData::Decimal(r)) => {
                let l = l as f64;
                match op {
                    "<"  => return EvalResult::Value(self.bool_ref(l < r)),
                    ">"  => return EvalResult::Value(self.bool_ref(l > r)),
                    "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
                    ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Division by zero"); }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Modulus by zero"); }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported here", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Decimal(l), ObjectData::Integer(r)) => {
                let r = r as f64;
                match op {
                    "<"  => return EvalResult::Value(self.bool_ref(l < r)),
                    ">"  => return EvalResult::Value(self.bool_ref(l > r)),
                    "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
                    ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
                    _ => {}
                }
                let result = match op {
                    "+" => ObjectData::Decimal(l + r),
                    "-" => ObjectData::Decimal(l - r),
                    "*" => ObjectData::Decimal(l * r),
                    "/" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Division by zero"); }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { return self.rt_err_kind("DivisionByZero", "Modulus by zero"); }
                        ObjectData::Decimal(l % r)
                    }
                    "**" => ObjectData::Decimal(l.powf(r)),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported here", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }

            (ObjectData::Str(l), ObjectData::Str(r)) => {
                match op {
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
                    "<"  => return EvalResult::Value(self.bool_ref(l < r)),
                    ">"  => return EvalResult::Value(self.bool_ref(l > r)),
                    "<=" => return EvalResult::Value(self.bool_ref(l <= r)),
                    ">=" => return EvalResult::Value(self.bool_ref(l >= r)),
                    "+"  => return EvalResult::Value(self.alloc(ObjectData::Str(l + &r))),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between strings", op);
                        return EvalResult::Error;
                    }
                }
            }
            (ObjectData::Str(s), ObjectData::Integer(n)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", s, n)))),
                    "*" => {
                        if n < 0 { eprintln!("❌ ERROR: Cannot repeat a string with a negative n"); return EvalResult::Error; }
                        if n > 10_000_000 { eprintln!("❌ ERROR: String repeat count {} exceeds maximum (10,000,000)", n); return EvalResult::Error; }
                        return EvalResult::Value(self.alloc(ObjectData::Str(s.repeat(n as usize))));
                    }
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and Integer", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Integer(n), ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", n, s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between Integer and String", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Str(s), ObjectData::Decimal(d)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", s, format_decimal(d))))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and Decimal", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Decimal(d), ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", format_decimal(d), s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between Decimal and String", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Str(s), ObjectData::Boolean(b)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", s, b)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and Boolean", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Boolean(b), ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", b, s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between Boolean and String", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Str(s), ObjectData::Null) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}null", s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and Null", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Null, ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("null{}", s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between Null and String", op); return EvalResult::Error; }
                }
            }
            // String concatenation with a DateTime renders its ISO 8601 form,
            // matching how int/decimal/bool concatenate.
            (ObjectData::Str(s), ObjectData::DateTime { epoch_ms, utc }) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", s, crate::region::format_datetime(epoch_ms, utc))))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between String and DateTime", op); return EvalResult::Error; }
                }
            }
            (ObjectData::DateTime { epoch_ms, utc }, ObjectData::Str(s)) => {
                match op {
                    "==" => return EvalResult::Value(self.false_ref),
                    "!=" => return EvalResult::Value(self.true_ref),
                    "+" => return EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", crate::region::format_datetime(epoch_ms, utc), s)))),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between DateTime and String", op); return EvalResult::Error; }
                }
            }
            (ObjectData::Boolean(l), ObjectData::Boolean(r)) => {
                match op {
                    "==" => return EvalResult::Value(self.bool_ref(l == r)),
                    "!=" => return EvalResult::Value(self.bool_ref(l != r)),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between booleans (use && / ||)", op); return EvalResult::Error; }
                }
            }
            // ── EnumVariant equality ─────────────────────────────────────────
            (ObjectData::EnumVariant { enum_name: en1, variant: v1 },
             ObjectData::EnumVariant { enum_name: en2, variant: v2 }) => {
                let eq = en1 == en2 && v1 == v2;
                match op {
                    "==" => return EvalResult::Value(self.bool_ref(eq)),
                    "!=" => return EvalResult::Value(self.bool_ref(!eq)),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported between enum variants", op); return EvalResult::Error; }
                }
            }

            (left, right) => {
                // ── String + instance: use op_str (consistent with interpolation
                // and array display, B-57/B-58). Checked before op_add so
                // `money + "x"` formats instead of calling op_add with a string.
                if op == "+" {
                    let left_opstr = if let ObjectData::Instance { ref class_name, .. } = left {
                        self.find_method(class_name, "op_str").map(|_| class_name.clone())
                    } else { None };
                    let right_opstr = if let ObjectData::Instance { ref class_name, .. } = right {
                        self.find_method(class_name, "op_str").map(|_| class_name.clone())
                    } else { None };

                    if let (ObjectData::Str(s), Some(cn)) = (&left, &right_opstr) {
                        let prefix = s.clone();
                        let cn = cn.clone();
                        let inst_ref = self.alloc(right);
                        return match self.call_op_method(inst_ref, &cn, "op_str", vec![], line, column) {
                            EvalResult::Value(r) => {
                                let rs = self.display(r);
                                EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", prefix, rs))))
                            }
                            other => other,
                        };
                    }
                    if let (Some(cn), ObjectData::Str(s)) = (&left_opstr, &right) {
                        let suffix = s.clone();
                        let cn = cn.clone();
                        let inst_ref = self.alloc(left);
                        return match self.call_op_method(inst_ref, &cn, "op_str", vec![], line, column) {
                            EvalResult::Value(r) => {
                                let ls = self.display(r);
                                EvalResult::Value(self.alloc(ObjectData::Str(format!("{}{}", ls, suffix))))
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
                    let inst_ref  = self.alloc(left);
                    let arg_ref   = self.alloc(right);
                    let arg_owned = self.extract(arg_ref);
                    return self.call_op_method(inst_ref, &class_name, method_name, vec![arg_owned], line, column);
                }

                // Cross-type equality: different types are never equal
                if op == "==" { return EvalResult::Value(self.false_ref); }
                if op == "!=" { return EvalResult::Value(self.true_ref); }
                eprintln!(
                    "❌ ERROR: Type mismatch — operator '{}' cannot be applied between '{}' and '{}' - [{}:{}]",
                    op, left_type, right_type, line, column
                );
                self.print_call_stack();
                EvalResult::Error
            }
        }
    }

}
