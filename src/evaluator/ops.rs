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
                        None => { eprintln!("❌ ERROR: Integer overflow"); return EvalResult::Error; }
                    },
                    "-" => match l.checked_sub(r) {
                        Some(v) => v,
                        None => { eprintln!("❌ ERROR: Integer overflow"); return EvalResult::Error; }
                    },
                    "*" => match l.checked_mul(r) {
                        Some(v) => v,
                        None => { eprintln!("❌ ERROR: Integer overflow"); return EvalResult::Error; }
                    },
                    "/" => {
                        if r == 0 { eprintln!("❌ ERROR: Division by zero"); return EvalResult::Error; }
                        match l.checked_div(r) {
                            Some(v) => v,
                            None => { eprintln!("❌ ERROR: Integer overflow"); return EvalResult::Error; }
                        }
                    }
                    "%" => {
                        if r == 0 { eprintln!("❌ ERROR: Modulus operator by zero"); return EvalResult::Error; }
                        match l.checked_rem(r) {
                            Some(v) => v,
                            None => { eprintln!("❌ ERROR: Modulo overflow (i64::MIN % -1 is undefined)"); return EvalResult::Error; }
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
                        if r == 0.0 { eprintln!("❌ ERROR: Division by zero"); return EvalResult::Error; }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { eprintln!("❌ ERROR: Modulus by zero"); return EvalResult::Error; }
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
                        if r == 0.0 { eprintln!("❌ ERROR: Division by zero"); return EvalResult::Error; }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { eprintln!("❌ ERROR: Modulus by zero"); return EvalResult::Error; }
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
                        if r == 0.0 { eprintln!("❌ ERROR: Division by zero"); return EvalResult::Error; }
                        ObjectData::Decimal(l / r)
                    }
                    "%" => {
                        if r == 0.0 { eprintln!("❌ ERROR: Modulus by zero"); return EvalResult::Error; }
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
