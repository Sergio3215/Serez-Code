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
    pub(super) fn eval_new_interface(&mut self, new_expr: &ast::NewExpression, iface_fields: Vec<ast::InterfaceField>) -> EvalResult {
        let provided = match &new_expr.args {
            ast::NewArgs::Fields(f) => f.clone(),
            ast::NewArgs::Positional(_) => {
                eprintln!("❌ ERROR: Interface '{}' must be instantiated with {{ field: value }} syntax", new_expr.class_name);
                return EvalResult::Error;
            }
        };

        // Check for extra fields not declared in the interface
        for (provided_name, _) in &provided {
            if !iface_fields.iter().any(|f| &f.name == provided_name) {
                eprintln!("❌ ERROR: Field '{}' is not declared in interface '{}'", provided_name, new_expr.class_name);
                return EvalResult::Error;
            }
        }

        let mut fields: Vec<(String, OwnedValue)> = Vec::new();
        for iface_field in &iface_fields {
            let entry = provided.iter().find(|(n, _)| n == &iface_field.name);
            match entry {
                Some((_, expr)) => {
                    let val_ref = match self.eval_expression(expr) {
                        EvalResult::Value(r) => r,
                        other => return other,
                    };
                    if let Some(actual) = self.resolve(val_ref) {
                        if !type_matches(&iface_field.type_name, actual) {
                            eprintln!("❌ TYPE ERROR: Interface field '{}' expects '{}' but got '{}'",
                                iface_field.name, iface_field.type_name, actual.type_name());
                            return EvalResult::Error;
                        }
                    }
                    let owned = self.extract(val_ref);
                    fields.push((iface_field.name.clone(), owned));
                }
                None => {
                    eprintln!("❌ ERROR: Missing field '{}' when creating '{}'", iface_field.name, new_expr.class_name);
                    return EvalResult::Error;
                }
            }
        }

        EvalResult::Value(self.alloc(ObjectData::Instance {
            class_name: new_expr.class_name.clone(),
            fields,
        }))
    }

    pub(super) fn eval_new_class(&mut self, new_expr: &ast::NewExpression, class: StoredClass) -> EvalResult {
        // ── Abstract class check ──────────────────────────────────────────────
        if class.is_abstract {
            eprintln!("❌ ERROR: Cannot instantiate abstract class '{}'", new_expr.class_name);
            return EvalResult::Error;
        }

        let arg_exprs = match &new_expr.args {
            ast::NewArgs::Positional(a) => a.clone(),
            ast::NewArgs::Fields(_) => {
                eprintln!("❌ ERROR: Class '{}' uses positional arguments, not field syntax", new_expr.class_name);
                return EvalResult::Error;
            }
        };

        // Evaluate args before pushing scope
        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in &arg_exprs {
            match self.eval_expression(expr) {
                EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        // ── Class field defaults ──────────────────────────────────────────────
        // Evaluate default values for class fields and add to initial instance
        let mut initial_fields: Vec<(String, OwnedValue)> = Vec::new();
        for field in &class.fields {
            if let Some(ref default_expr) = field.default_value {
                match self.eval_expression(default_expr) {
                    EvalResult::Value(r) => {
                        let owned = self.extract(r);
                        initial_fields.push((field.name.clone(), owned));
                    }
                    other => return other,
                }
            }
        }

        // Allocate instance with default field values
        let instance_ref = self.alloc(ObjectData::Instance {
            class_name: new_expr.class_name.clone(),
            fields: initial_fields,
        });

        if let Some(ctor) = class.constructor {
            if arg_vals.len() != ctor.parameters.len() {
                eprintln!("❌ ERROR: Constructor '{}' expects {} arguments, got {}",
                    new_expr.class_name, ctor.parameters.len(), arg_vals.len());
                return EvalResult::Error;
            }

            self.scopes.push();
            self.scopes.declare("this".to_string(), instance_ref);

            for (i, param) in ctor.parameters.iter().enumerate() {
                let arg_ref = self.plant(arg_vals[i].clone());
                self.scopes.declare(param.name.clone(), arg_ref);
            }

            let old_class = self.constructing_class.replace(new_expr.class_name.clone());

            let mut body_error = false;
            let mut ctor_throw: Option<ObjectRef> = None;
            for stmt in &ctor.body.statements {
                match self.eval_statement(stmt) {
                    EvalResult::Error => { body_error = true; break; }
                    EvalResult::Return(_) => break,
                    EvalResult::Value(_) => {}
                    EvalResult::Throw(v) => { ctor_throw = Some(v); break; }
                    EvalResult::Break | EvalResult::Continue
                    | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                        eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                        body_error = true;
                        break;
                    }
                }
            }

            self.constructing_class = old_class;

            // Extract instance state before popping constructor scope
            let instance_owned = self.extract(instance_ref);
            let throw_owned = ctor_throw.map(|r| self.extract(r));
            self.scopes.pop();

            if body_error { return EvalResult::Error; }
            if let Some(owned) = throw_owned { return EvalResult::Throw(self.plant(owned)); }

            // Re-plant instance in outer context with updated fields
            let final_ref = self.plant(instance_owned);
            EvalResult::Value(final_ref)
        } else {
            if !arg_vals.is_empty() {
                eprintln!("❌ ERROR: Class '{}' has no constructor but received {} arguments",
                    new_expr.class_name, arg_vals.len());
                return EvalResult::Error;
            }
            EvalResult::Value(instance_ref)
        }
    }

    pub(super) fn eval_super_call(&mut self, args: &[ast::Expression]) -> EvalResult {
        let current_class = match &self.constructing_class {
            Some(c) => c.clone(),
            None => {
                eprintln!("❌ ERROR: super() called outside of a constructor");
                return EvalResult::Error;
            }
        };
        let parent_name = match self.class_registry.get(&current_class).and_then(|c| c.parent.clone()) {
            Some(p) => p,
            None => {
                eprintln!("❌ ERROR: Class '{}' has no parent to call super() on", current_class);
                return EvalResult::Error;
            }
        };
        let parent_ctor = match self.class_registry.get(&parent_name).and_then(|c| c.constructor.clone()) {
            Some(ctor) => ctor,
            None => return EvalResult::Value(self.null_ref), // parent has no constructor
        };

        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in args {
            match self.eval_expression(expr) {
                EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        let has_rest = parent_ctor.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required = parent_ctor.parameters.iter().filter(|p| !p.is_rest && p.default_value.is_none()).count();
        let max_pos  = if has_rest { usize::MAX } else { parent_ctor.parameters.len() };
        if arg_vals.len() < required || arg_vals.len() > max_pos {
            eprintln!("❌ ERROR: super() for '{}' expects {} arguments, got {}",
                parent_name, parent_ctor.parameters.len(), arg_vals.len());
            return EvalResult::Error;
        }

        // Execute parent constructor body — "this" is already bound in the current scope
        self.scopes.push();
        for (i, param) in parent_ctor.parameters.iter().enumerate() {
            if param.is_rest {
                let rest_refs: Vec<ObjectRef> = arg_vals[i..].iter()
                    .map(|v| self.plant(v.clone()))
                    .collect();
                let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_refs });
                self.scopes.declare(param.name.clone(), rest_ref);
                break;
            }
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else if let Some(default_expr) = &param.default_value {
                let default_expr = default_expr.clone();
                match self.eval_expression(&default_expr) {
                    EvalResult::Value(v) => v,
                    _ => self.null_ref,
                }
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let old_class = self.constructing_class.replace(parent_name);

        let mut error = false;
        let mut super_throw: Option<ObjectRef> = None;
        for stmt in &parent_ctor.body.statements {
            match self.eval_statement(stmt) {
                EvalResult::Error => { error = true; break; }
                EvalResult::Return(_) => break,
                EvalResult::Value(_) => {}
                EvalResult::Throw(v) => { super_throw = Some(v); break; }
                EvalResult::Break | EvalResult::Continue
                | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                    error = true;
                    break;
                }
            }
        }

        self.constructing_class = old_class;
        let throw_owned = super_throw.map(|r| self.extract(r));
        self.scopes.pop();

        if error { return EvalResult::Error; }
        if let Some(owned) = throw_owned { return EvalResult::Throw(self.plant(owned)); }
        EvalResult::Value(self.null_ref)
    }

    pub(super) fn eval_super_method_call(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        let current_class = match &self.executing_class {
            Some(c) => c.clone(),
            None => {
                eprintln!("❌ ERROR: super.{}() called outside of a class method", dot_call.method);
                return EvalResult::Error;
            }
        };

        let parent_name = match self.class_registry.get(&current_class).and_then(|c| c.parent.clone()) {
            Some(p) => p,
            None => {
                eprintln!("❌ ERROR: Class '{}' has no parent — cannot call super.{}()", current_class, dot_call.method);
                return EvalResult::Error;
            }
        };

        let method = match self.find_method(&parent_name, &dot_call.method) {
            Some(m) => m,
            None => {
                eprintln!("❌ ERROR: Parent class '{}' has no method '{}'", parent_name, dot_call.method);
                return EvalResult::Error;
            }
        };

        let this_ref = match self.scopes.lookup("this") {
            Some(r) => r,
            None => {
                eprintln!("❌ ERROR: super.{}() called with no 'this' in scope", dot_call.method);
                return EvalResult::Error;
            }
        };

        let mut arg_vals: Vec<OwnedValue> = Vec::new();
        for expr in &dot_call.arguments {
            match self.eval_expression(expr) {
                EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                other => return other,
            }
        }

        if arg_vals.len() != method.parameters.len() {
            eprintln!("❌ ERROR: Method '{}::{}' expects {} arguments, got {}",
                parent_name, dot_call.method, method.parameters.len(), arg_vals.len());
            return EvalResult::Error;
        }

        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(parent_name.clone());

        self.call_stack.push(CallFrame {
            name: format!("{}::{}", parent_name, dot_call.method),
            line: dot_call.line,
            column: dot_call.column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), this_ref);

        for (i, param) in method.parameters.iter().enumerate() {
            let arg_ref = self.plant(arg_vals[i].clone());
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let mut result_ref = self.null_ref;
        let mut error = false;
        let mut method_throw: Option<ObjectRef> = None;
        for stmt in &method.body.statements {
            match self.eval_statement(stmt) {
                EvalResult::Value(_) => {}
                EvalResult::Return(v) => { result_ref = v; break; }
                EvalResult::Throw(v)  => { method_throw = Some(v); break; }
                EvalResult::Error => { error = true; break; }
                EvalResult::Break | EvalResult::Continue
                | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                    error = true;
                    break;
                }
            }
        }

        let owned = self.extract(result_ref);
        let throw_owned = method_throw.map(|r| self.extract(r));
        self.call_depth -= 1;
        self.scopes.pop();
        self.call_stack.pop();
        self.executing_class = old_executing_class;

        if error { return EvalResult::Error; }
        if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }
        EvalResult::Value(self.plant(owned))
    }

    pub(super) fn eval_object_patch(&mut self, var_name: &str, patch: Vec<(String, ast::Expression)>) -> EvalResult {
        let obj_ref = match self.lookup_var(var_name) {
            Some(r) => r,
            None => {
                eprintln!("❌ ERROR: Undeclared variable '{}' in object patch", var_name);
                return EvalResult::Error;
            }
        };

        if let Some(ObjectData::Instance { class_name, mut fields }) = self.resolve(obj_ref).cloned() {
            // Validate against interface schema if it's an interface
            let schema = self.interface_registry.get(&class_name).cloned();

            for (field_name, expr) in patch {
                let val_ref = match self.eval_expression(&expr) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if let Some(ref schema_fields) = schema {
                    if let Some(iface_field) = schema_fields.iter().find(|f| f.name == field_name) {
                        if let Some(actual) = self.resolve(val_ref) {
                            if !type_matches(&iface_field.type_name, actual) {
                                eprintln!("❌ TYPE ERROR: Field '{}' expects '{}' but got '{}'",
                                    field_name, iface_field.type_name, actual.type_name());
                                return EvalResult::Error;
                            }
                        }
                    }
                }
                let owned = self.extract(val_ref);
                if let Some(f) = fields.iter_mut().find(|(n, _)| n == &field_name) {
                    f.1 = owned;
                } else {
                    fields.push((field_name, owned));
                }
            }

            match obj_ref.region {
                RegionId::Global => self.global_arena.update(obj_ref.index, ObjectData::Instance { class_name, fields }),
                RegionId::Scoped => self.scopes.arena.update(obj_ref.index, ObjectData::Instance { class_name, fields }),
            }
            EvalResult::Value(self.null_ref)
        } else {
            eprintln!("❌ ERROR: '{}' is not an interface instance — cannot use patch syntax", var_name);
            EvalResult::Error
        }
    }

    pub(super) fn eval_instance_dot(
        &mut self,
        obj_ref: ObjectRef,
        class_name: String,
        fields: Vec<(String, OwnedValue)>,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        let method_name = &dot_call.method;

        // Field read: no parens and no args and field exists → return value (not call)
        if !dot_call.has_parens && dot_call.arguments.is_empty() {
            if let Some((_, owned)) = fields.iter().find(|(n, _)| n == method_name) {
                let owned = owned.clone();
                return EvalResult::Value(self.plant(owned));
            }
            // Getter: no parens, no field → look for `get prop()`
            if let Some(getter) = self.find_getter(&class_name, method_name) {
                return self.invoke_method(obj_ref, &class_name, &getter, vec![], dot_call.line, dot_call.column);
            }
        }

        // Method dispatch: walk inheritance chain
        let method = self.find_method(&class_name, method_name);
        match method {
            Some(m) => {
                let args_exprs = dot_call.arguments.clone();
                let mut arg_vals: Vec<OwnedValue> = Vec::new();
                for expr in &args_exprs {
                    match self.eval_expression(expr) {
                        EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                        other => return other,
                    }
                }
                self.invoke_method(obj_ref, &class_name, &m, arg_vals, dot_call.line, dot_call.column)
            }
            None => {
                // Fallback: toString() is available on all instance types
                if method_name == "toString" {
                    let s = self.display(obj_ref);
                    return EvalResult::Value(self.alloc(ObjectData::Str(s)));
                }
                // Fallback: field holds a callable function (this.fn_field(args))
                if let Some((_, owned)) = fields.iter().find(|(n, _)| n == method_name) {
                    let owned = owned.clone();
                    let fn_ref = self.plant(owned);
                    let mut arg_vals = Vec::new();
                    for arg_expr in &dot_call.arguments {
                        match self.eval_expression(arg_expr) {
                            EvalResult::Value(r) => arg_vals.push(self.extract(r)),
                            other => return other,
                        }
                    }
                    return self.call_function(fn_ref, arg_vals);
                }
                eprintln!("❌ ERROR: '{}' has no field or method named '{}'", class_name, method_name);
                EvalResult::Error
            }
        }
    }

    // Walk the inheritance chain to find a method
    pub(super) fn find_method(&self, class_name: &str, method_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.get(method_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    pub(super) fn find_getter(&self, class_name: &str, prop_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.getters.get(prop_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    pub(super) fn find_setter(&self, class_name: &str, prop_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.setters.get(prop_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    // Shared helper: invoke a ClassMethod on an instance with pre-evaluated arg values.
    pub(super) fn invoke_method(
        &mut self,
        obj_ref: ObjectRef,
        class_name: &str,
        m: &ast::ClassMethod,
        arg_vals: Vec<OwnedValue>,
        call_line: usize,
        call_column: usize,
    ) -> EvalResult {
        let method_name = &m.name;

        // arity check — account for default parameter values and rest params
        let has_rest_m = m.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required_count = m.parameters.iter().filter(|p| !p.is_rest && p.default_value.is_none()).count();
        let max_count = if has_rest_m { usize::MAX } else { m.parameters.len() };
        if arg_vals.len() < required_count || arg_vals.len() > max_count {
            let expected_str = if has_rest_m {
                format!("at least {}", required_count)
            } else if required_count == max_count {
                format!("{}", required_count)
            } else {
                format!("{}-{}", required_count, max_count)
            };
            eprintln!("❌ ERROR: Method '{}' expects {} argument(s), got {}",
                method_name, expected_str, arg_vals.len());
            return EvalResult::Error;
        }

        if !m.is_public && self.executing_class.as_deref() != Some(class_name) {
            eprintln!("❌ ERROR: Method '{}' is private and cannot be called externally", method_name);
            return EvalResult::Error;
        }

        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(class_name.to_string());

        self.call_stack.push(CallFrame {
            name: format!("{}::{}", class_name, method_name),
            line: call_line,
            column: call_column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), obj_ref);

        for (i, param) in m.parameters.iter().enumerate() {
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else if let Some(default_expr) = &param.default_value {
                let default_expr = default_expr.clone();
                match self.eval_expression(&default_expr) {
                    EvalResult::Value(v) => v,
                    _ => self.null_ref,
                }
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let mut result_ref = self.null_ref;
        let mut error = false;
        let mut method_throw: Option<ObjectRef> = None;
        for stmt in &m.body.statements {
            match self.eval_statement(stmt) {
                EvalResult::Value(_) => {}
                EvalResult::Return(v) => { result_ref = v; break; }
                EvalResult::Throw(v)  => { method_throw = Some(v); break; }
                EvalResult::Error => { error = true; break; }
                EvalResult::Break | EvalResult::Continue
                | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                    error = true;
                    break;
                }
            }
        }

        let owned = self.extract(result_ref);
        let throw_owned = method_throw.map(|r| self.extract(r));
        self.call_depth -= 1;
        self.scopes.pop();
        self.call_stack.pop();
        self.executing_class = old_executing_class;

        if error { return EvalResult::Error; }
        if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }

        let result = self.plant(owned);

        if let Some(ref rt) = m.return_type {
            let actual = self.resolve(result).unwrap();
            if !type_matches(rt, actual) {
                eprintln!("❌ TYPE ERROR: Method '{}' declared return '{}' but returned '{}'",
                    method_name, rt, actual.type_name());
                return EvalResult::Error;
            }
        }

        EvalResult::Value(result)
    }

    // ── Array methods ─────────────────────────────────────────────────────────

}
