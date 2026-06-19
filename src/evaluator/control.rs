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
    pub(super) fn eval_str_arg(&mut self, expr: &ast::Expression) -> Option<String> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => Some(s),
                _ => { eprintln!("❌ ERROR: Expected string argument"); None }
            },
            _ => None,
        }
    }

    pub(super) fn eval_int_arg(&mut self, expr: &ast::Expression) -> Option<i64> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Integer(i)) => Some(i),
                _ => { eprintln!("❌ ERROR: Expected int argument"); None }
            },
            _ => None,
        }
    }

    // ── switch ────────────────────────────────────────────────────────────────

    pub(super) fn eval_switch(&mut self, sw: &ast::SwitchStatement) -> EvalResult {
        let val_ref = match self.eval_expression(&sw.value) {
            EvalResult::Value(v) => v,
            other => return other,
        };
        let val_data = match self.resolve(val_ref).cloned() {
            Some(d) => d,
            None => return EvalResult::Error,
        };

        for case in &sw.cases {
            for case_expr in &case.values {
                let case_ref = match self.eval_expression(case_expr) {
                    EvalResult::Value(v) => v,
                    other => return other,
                };
                let case_data = match self.resolve(case_ref).cloned() {
                    Some(d) => d,
                    None => return EvalResult::Error,
                };
                if self.values_equal(&val_data, &case_data) {
                    return self.eval_block(&case.body);
                }
            }
        }

        if let Some(ref default_block) = sw.default {
            return self.eval_block(default_block);
        }

        EvalResult::Value(self.null_ref)
    }

    pub(super) fn values_equal(&self, a: &ObjectData, b: &ObjectData) -> bool {
        // A DateField compares as its integer value (e.g. switch on date.month).
        let a_norm; let b_norm;
        let a = match a { ObjectData::DateField { value, .. } => { a_norm = ObjectData::Integer(*value); &a_norm } _ => a };
        let b = match b { ObjectData::DateField { value, .. } => { b_norm = ObjectData::Integer(*value); &b_norm } _ => b };
        match (a, b) {
            (ObjectData::Integer(x),  ObjectData::Integer(y))  => x == y,
            // Two DateTimes are equal when they denote the same instant.
            (ObjectData::DateTime { epoch_ms: x, .. }, ObjectData::DateTime { epoch_ms: y, .. }) => x == y,
            // Exact decimal, by value (scale ignored); int mixes in exactly.
            (ObjectData::Dec(x), ObjectData::Dec(y)) => x == y,
            (ObjectData::Dec(x), ObjectData::Integer(y)) => *x == rust_decimal::Decimal::from(*y),
            (ObjectData::Integer(x), ObjectData::Dec(y)) => rust_decimal::Decimal::from(*x) == *y,
            (ObjectData::Decimal(x),  ObjectData::Decimal(y))  => x == y,
            // Cross-type numeric: same coercion that == uses in infix expressions
            (ObjectData::Decimal(x),  ObjectData::Integer(y))  => *x == (*y as f64),
            (ObjectData::Integer(x),  ObjectData::Decimal(y))  => (*x as f64) == *y,
            (ObjectData::Str(x),      ObjectData::Str(y))      => x == y,
            (ObjectData::Boolean(x),  ObjectData::Boolean(y))  => x == y,
            (ObjectData::Null,        ObjectData::Null)         => true,
            (ObjectData::EnumVariant { enum_name: en1, variant: v1 },
             ObjectData::EnumVariant { enum_name: en2, variant: v2 }) => en1 == en2 && v1 == v2,
            _ => false,
        }
    }

    // ── try / catch / finally ─────────────────────────────────────────────────

    pub(super) fn eval_try(&mut self, try_stmt: &ast::TryStatement) -> EvalResult {
        let body_result = self.eval_block(&try_stmt.body);

        let result_after_catch = match body_result {
            EvalResult::Throw(thrown_ref) => {
                if let Some(ref catch_block) = try_stmt.catch_body {
                    self.scopes.push();
                    if let Some(ref var_name) = try_stmt.catch_var {
                        self.scopes.declare(var_name.clone(), thrown_ref);
                    }
                    // Run catch body statement-by-statement (same pattern as eval_call)
                    // so we can extract the result BEFORE the scope pop.
                    let mut catch_val = self.null_ref;
                    let mut catch_return:   Option<ObjectRef> = None;
                    let mut catch_throw:    Option<ObjectRef> = None;
                    let mut catch_error    = false;
                    let mut catch_break    = false;
                    let mut catch_continue = false;
                    let mut catch_break_label: Option<String> = None;
                    let mut catch_continue_label: Option<String> = None;
                    for s in &catch_block.statements {
                        match self.eval_statement(s) {
                            EvalResult::Value(v)   => catch_val = v,
                            EvalResult::Return(v)  => { catch_return   = Some(v); break; }
                            EvalResult::Throw(v)   => { catch_throw    = Some(v); break; }
                            EvalResult::Error      => { catch_error    = true;    break; }
                            EvalResult::Break      => { catch_break    = true;    break; }
                            EvalResult::Continue   => { catch_continue = true;    break; }
                            EvalResult::BreakLabel(l)    => { catch_break_label    = Some(l); break; }
                            EvalResult::ContinueLabel(l) => { catch_continue_label = Some(l); break; }
                        }
                    }
                    // Extract BEFORE pop so refs remain valid
                    let primary = catch_return.or(catch_throw).unwrap_or(catch_val);
                    let owned = self.extract(primary);
                    self.scopes.pop();
                    if catch_error             { EvalResult::Error }
                    else if catch_break        { EvalResult::Break }
                    else if catch_continue     { EvalResult::Continue }
                    else if let Some(l) = catch_break_label    { EvalResult::BreakLabel(l) }
                    else if let Some(l) = catch_continue_label { EvalResult::ContinueLabel(l) }
                    else if catch_return.is_some() { EvalResult::Return(self.plant(owned)) }
                    else if catch_throw.is_some()  { EvalResult::Throw(self.plant(owned)) }
                    else                           { EvalResult::Value(self.plant(owned)) }
                } else {
                    EvalResult::Throw(thrown_ref) // no catch — re-throw after finally
                }
            }
            other => other,
        };

        // finally always runs — throw/return/error from it override the try/catch result
        if let Some(ref finally_block) = try_stmt.finally_body {
            match self.eval_block(finally_block) {
                EvalResult::Throw(v)  => return EvalResult::Throw(v),
                EvalResult::Return(v) => return EvalResult::Return(v),
                EvalResult::Error     => return EvalResult::Error,
                _ => {} // Value / Break / Continue — preserve result_after_catch
            }
        }

        result_after_catch
    }
}
