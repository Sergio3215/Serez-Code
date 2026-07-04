#![allow(unused_imports)]
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use super::{EvalResult, StoredClass, CallFrame, type_matches, obj_data_to_key_str, owned_to_key_str,
            obj_data_eq, format_decimal, json_stringify_owned, json_parse,
            operator_to_method_name};

impl super::Evaluator {
    /// Writes a mutated collection (`obj_ref`) back into the instance field it
    /// was read from — the `instance.field.mutate(...)` pattern. Shared by the
    /// generic dot-call path and the slot fast paths for Set/Array (value
    /// semantics plant a copy on field read; without this, the mutation would
    /// be lost).
    pub(super) fn apply_field_writeback(&mut self, inner_obj_expr: &Expression, field_name: &str, obj_ref: ObjectRef) {
        if let EvalResult::Value(inst_ref) = self.eval_expression(inner_obj_expr) {
            if let Some(ObjectData::Instance { class_name, mut fields }) = self.resolve(inst_ref).cloned() {
                let updated = self.extract(obj_ref);
                if let Some(f) = fields.iter_mut().find(|(n, _)| n == field_name) {
                    f.1 = updated;
                }
                match inst_ref.region {
                    RegionId::Global => self.global_arena.update(inst_ref.index, ObjectData::Instance { class_name, fields }),
                    RegionId::Scoped => self.scopes.arena.update(inst_ref.index, ObjectData::Instance { class_name, fields }),
                }
            }
        }
    }

    pub(super) fn eval_expression(&mut self, expr: &Expression) -> EvalResult {
        match expr {
            Expression::Integer(i) => EvalResult::Value(self.int_ref(*i)),
            Expression::Decimal(d) => EvalResult::Value(self.alloc(ObjectData::Decimal(*d))),
            Expression::Dec(d) => EvalResult::Value(self.alloc(ObjectData::Dec(*d))),
            Expression::String(s) => EvalResult::Value(self.alloc(ObjectData::Str(s.clone()))),
            Expression::Boolean(b) => EvalResult::Value(self.alloc(ObjectData::Boolean(*b))),
            Expression::Null => EvalResult::Value(self.null_ref),

            Expression::Identifier(name) => match self.lookup_var(name) {
                Some(r) => EvalResult::Value(r),
                None => {
                    let n = name.clone();
                    self.rt_err_kind("ReferenceError", format!("Variable not found: {}", n))
                }
            },

            Expression::FunctionLiteral(func_lit) => {
                let captured = self.capture_lambda_env(&func_lit.body); // snapshot incl. referenced globals (B-83)
                let func_data = ObjectData::Function {
                    return_type: func_lit.return_type.clone(),
                    parameters: Rc::new(func_lit.parameters.clone()),
                    body: Rc::new(func_lit.body.clone()),
                    captured: Rc::new(captured),
                    is_generator: func_lit.is_generator,
                };
                EvalResult::Value(self.alloc(func_data))
            }

            Expression::Lambda(lambda) => {
                use crate::ast::{LambdaBody, Parameter, BlockStatement, Statement, ReturnStatement};
                let params: Vec<Parameter> = lambda.params.iter()
                    .map(|n| Parameter { name: n.clone(), type_name: None, is_rest: false, default_value: None })
                    .collect();
                let body = match &lambda.body {
                    LambdaBody::Block(b) => b.clone(),
                    LambdaBody::Expr(e) => BlockStatement {
                        statements: vec![Statement::Return(ReturnStatement {
                            return_value: *e.clone(),
                        })],
                    },
                };
                let captured = self.capture_lambda_env(&body); // snapshot incl. referenced globals (B-83)
                EvalResult::Value(self.alloc(ObjectData::Function {
                    return_type: None,
                    parameters: Rc::new(params),
                    body: Rc::new(body),
                    captured: Rc::new(captured),
                    is_generator: false,
                }))
            }

            Expression::InterpolatedString(parts) => {
                let mut result = String::new();
                for part in parts {
                    match part {
                        ast::StringPart::Literal(s) => result.push_str(s),
                        ast::StringPart::Expr(expr) => {
                            match self.eval_expression(expr) {
                                EvalResult::Value(r) => {
                                    match self.fmt_value(r) {
                                        Ok(s)  => result.push_str(&s),
                                        Err(e) => return e,
                                    }
                                }
                                other => return other,
                            }
                        }
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            Expression::Call(call_expr) => {
                // Built-in global functions (intercept before variable lookup)
                if let Expression::Identifier(name) = call_expr.function.as_ref() {
                    match name.as_str() {
                        "parseInt"    => return self.eval_parse_int(&call_expr.arguments),
                        "parseDecimal"=> return self.eval_parse_decimal(&call_expr.arguments),
                        "readLine"    => return self.eval_read_line(&call_expr.arguments),
                        "fetch" if self.lookup_var("fetch").is_none() => return self.eval_fetch(&call_expr.arguments),
                        "super"       => return self.eval_super_call(&call_expr.arguments),
                        "assert"      => return self.eval_assert(&call_expr.arguments),
                        "type_of"     => return self.eval_type_of(&call_expr.arguments),
                        "abs" | "sqrt" | "floor" | "ceil" | "round"
                        | "min" | "max" | "pow" | "log" | "log2" | "log10"
                            => return self.eval_math_builtin(name, &call_expr.arguments),
                        "time"  => return self.eval_builtin_time(),
                        "env"   => return self.eval_builtin_env(&call_expr.arguments),
                        "exit"  => return self.eval_builtin_exit(&call_expr.arguments),
                        _ => {}
                    }
                    // native fn dispatch: if name is registered as a native function but has no
                    // variable binding, it must be one of the built-in natives listed above; if it
                    // reached here there is no Rust implementation for it.
                    if self.native_fns.contains(name) && self.lookup_var(name).is_none() {
                        let n = name.clone();
                        return self.rt_err_kind("TypeError", format!("native function '{}' has no Rust implementation registered", n));
                    }
                }

                if self.call_depth >= 1000 {
                    eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
                    return EvalResult::Error;
                }

                let func_ref = match self.eval_expression(&call_expr.function) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };

                let call_name = match call_expr.function.as_ref() {
                    Expression::Identifier(name) => name.clone(),
                    _ => "<anonymous>".to_string(),
                };
                let call_line = call_expr.line;
                let call_col = call_expr.column;
                self.call_stack.push(CallFrame {
                    name: call_name,
                    line: call_line,
                    column: call_col,
                });
                self.call_depth += 1;

                self.scopes.push();

                let mut func_data = self.resolve(func_ref).cloned();
                // A non-callable shadow (e.g. a parameter named like an outer
                // function) must not hide that function from a CALL — reads
                // still see the shadow, but `name(...)` falls back to the
                // nearest binding that actually holds a function.
                if !matches!(func_data, Some(ObjectData::Function { .. })) {
                    if let Expression::Identifier(name) = call_expr.function.as_ref() {
                        if let Some(fref) = self.lookup_callable(name) {
                            func_data = self.resolve(fref).cloned();
                        }
                    }
                }
                let (return_type, parameters, body, captured, is_generator) = match func_data {
                    Some(ObjectData::Function {
                        return_type,
                        parameters,
                        body,
                        captured,
                        is_generator,
                    }) => (return_type, parameters, body, captured, is_generator),
                    _ => {
                        // Raise BEFORE unwinding so the printed call stack still
                        // shows the failing frame; state is restored either way,
                        // so a catching try/catch sees a consistent evaluator.
                        let err = self.rt_err_kind("TypeError", "Attempt to call a non-function");
                        self.scopes.pop();
                        self.call_depth -= 1;
                        self.call_stack.pop();
                        return err;
                    }
                };

                let mut arg_refs = Vec::new();
                for arg in &call_expr.arguments {
                    // Spread: ...expr expands an array into the argument list
                    if let Expression::Spread(inner) = arg {
                        let spread_ref = match self.eval_expression(inner) {
                            EvalResult::Value(r) => r,
                            _ => { self.scopes.pop(); self.call_depth -= 1; self.call_stack.pop(); return EvalResult::Error; }
                        };
                        match self.resolve(spread_ref).cloned() {
                            Some(ObjectData::Array { elements: spread_elems, .. }) => {
                                for elem in spread_elems {
                                    let planted = self.plant(elem);
                                    arg_refs.push(planted);
                                }
                            }
                            _ => {
                                let err = self.rt_err_kind("TypeError", "Spread in function call requires an array");
                                self.scopes.pop(); self.call_depth -= 1; self.call_stack.pop();
                                return err;
                            }
                        }
                        continue;
                    }
                    match self.eval_expression(arg) {
                        EvalResult::Value(r) => arg_refs.push(r),
                        _ => {
                            self.scopes.pop();
                            self.call_depth -= 1;
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                    }
                }

                // Check for rest parameter (last param with is_rest=true)
                let has_rest = parameters.last().map(|p| p.is_rest).unwrap_or(false);
                let required_count = parameters.iter().filter(|p| !p.is_rest && p.default_value.is_none()).count();
                let min_params = required_count;
                let max_params = if has_rest { usize::MAX } else { parameters.len() };

                if arg_refs.len() < min_params || arg_refs.len() > max_params {
                    let expected_str = if has_rest {
                        format!("at least {}", min_params)
                    } else if min_params == max_params {
                        format!("{}", min_params)
                    } else {
                        format!("{}-{}", min_params, max_params)
                    };
                    // rt_err_kind prints the message + call stack (uncaught case)
                    // with the failing frame still on it; then unwind.
                    let err = self.rt_err_kind("TypeError", format!(
                        "Function expected {} argument(s), got {}",
                        expected_str, arg_refs.len()
                    ));
                    self.scopes.pop();
                    self.call_depth -= 1;
                    self.call_stack.pop();
                    return err;
                }

                for (i, param) in parameters.iter().enumerate() {
                    if param.is_rest { break; }
                    if i >= arg_refs.len() { break; } // default will be used
                    let arg_ref = arg_refs[i];
                    if let Some(expected_type) = &param.type_name {
                        let actual_data = self.resolve(arg_ref).unwrap();
                        let is_valid = type_matches(expected_type.as_str(), actual_data);
                        if !is_valid {
                            eprintln!(
                                "❌ TYPE ERROR: Parameter '{}' expected '{}' but received '{}'.",
                                param.name, expected_type, actual_data.type_name()
                            );
                            self.print_call_stack();
                            self.scopes.pop();
                            self.call_depth -= 1;
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                    }
                }

                // Bind captured environment first — params shadow same-named captures
                for (name, cap_ref) in captured.iter() {
                    self.scopes.declare(name.clone(), *cap_ref);
                }

                for (i, param) in parameters.iter().enumerate() {
                    if param.is_rest {
                        // Collect remaining args into an array
                        let rest_elems: Vec<OwnedValue> = arg_refs[i.min(arg_refs.len())..].iter()
                            .map(|&r| self.extract(r))
                            .collect();
                        let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_elems });
                        self.scopes.declare(param.name.clone(), rest_ref);
                        break;
                    }
                    let local_ref = if i < arg_refs.len() {
                        let arg_data = self.resolve(arg_refs[i]).unwrap().clone();
                        self.alloc(arg_data)
                    } else if let Some(default_expr) = &param.default_value {
                        let default_expr = default_expr.clone();
                        match self.eval_expression(&default_expr) {
                            EvalResult::Value(v) => v,
                            _ => self.null_ref,
                        }
                    } else {
                        self.null_ref
                    };
                    self.scopes.declare(param.name.clone(), local_ref);
                }

                // Generator: save outer collector, install a fresh one
                let prev_collector = if is_generator {
                    let prev = self.yield_collector.take();
                    self.yield_collector = Some(Vec::new());
                    prev
                } else {
                    None
                };

                let mut result_ref = self.null_ref;
                let mut early_throw: Option<OwnedValue> = None;
                let mut early_error = false;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(_) => {} // implicit — function result is null unless explicit return
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v) => {
                            early_throw = Some(self.extract(v));
                            break;
                        }
                        EvalResult::Error => {
                            early_error = true;
                            break;
                        }
                        EvalResult::Break | EvalResult::Continue
                        | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                            eprintln!("❌ ERROR: 'break'/'continue' cannot be used outside of a loop");
                            early_error = true;
                            break;
                        }
                    }
                }

                // Generator: collect yielded values before popping scope
                if is_generator {
                    let collected = self.yield_collector.take().unwrap_or_default();
                    self.yield_collector = prev_collector;
                    self.scopes.pop();
                    self.call_depth -= 1;
                    self.call_stack.pop();
                    if early_error { return EvalResult::Error; }
                    if let Some(thrown) = early_throw { return EvalResult::Throw(self.plant(thrown)); }
                    let arr_ref = self.alloc(ObjectData::Array { element_type: None, elements: collected });
                    return EvalResult::Value(arr_ref);
                }

                if early_error {
                    self.scopes.pop(); self.call_depth -= 1; self.call_stack.pop();
                    return EvalResult::Error;
                }
                if let Some(thrown) = early_throw {
                    self.scopes.pop(); self.call_depth -= 1; self.call_stack.pop();
                    return EvalResult::Throw(self.plant(thrown));
                }

                // Deep-extract ANTES del pop — preserva elementos de arrays anidados
                let owned = self.extract(result_ref);

                self.scopes.pop(); // Flash Scope: destrucción instantánea de temporales
                self.call_depth -= 1;
                self.call_stack.pop();
                let result_ref = self.plant(owned);

                if let Some(expected_ret) = &return_type {
                    let actual_data = self.resolve(result_ref).unwrap();
                    let is_valid = type_matches(expected_ret.as_str(), actual_data);
                    if !is_valid {
                        eprintln!(
                            "❌ TYPE ERROR: Function expected to return '{}' but returned another type.",
                            expected_ret
                        );
                        self.print_call_stack();
                        return EvalResult::Error;
                    }
                }

                EvalResult::Value(result_ref)
            }

            Expression::ArrayLiteral(arr) => {
                let mut owned_elems = Vec::new();
                for el in &arr.elements {
                    // Spread: ...expr expands an array into this array
                    if let Expression::Spread(inner) = el {
                        let spread_ref = match self.eval_expression(inner) {
                            EvalResult::Value(r) => r,
                            EvalResult::Throw(v) => return EvalResult::Throw(v),
                            _ => return EvalResult::Error,
                        };
                        match self.resolve(spread_ref).cloned() {
                            Some(ObjectData::Array { elements: spread_elems, .. }) => {
                                for elem in spread_elems {
                                    owned_elems.push(elem);
                                }
                            }
                            _ => {
                                eprintln!("❌ ERROR: Spread operator requires an array");
                                return EvalResult::Error;
                            }
                        }
                        continue;
                    }
                    match self.eval_expression(el) {
                        EvalResult::Value(r) => {
                            if let Some(ref et) = arr.element_type {
                                let data = self.resolve(r).unwrap();
                                if !type_matches(et, data) {
                                    eprintln!(
                                        "❌ TYPE ERROR: Array declared as [{}] but element has type '{}'",
                                        et, data.type_name()
                                    );
                                    return EvalResult::Error;
                                }
                            }
                            let owned = self.extract(r);
                            owned_elems.push(owned);
                        }
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: arr.element_type.clone(),
                    elements: owned_elems,
                }))
            }

            Expression::If(if_expr) => {
                let condition_ref = match self.eval_expression(&if_expr.condition) {
                    EvalResult::Value(r) => r,
                    EvalResult::Return(v) => return EvalResult::Return(v),
                    other => return other,
                };

                let condition_data = self.resolve(condition_ref).unwrap().clone();
                if self.is_truthy(&condition_data) {
                    self.eval_block(&if_expr.consequence)
                } else if let Some(alt) = &if_expr.alternative {
                    self.eval_block(alt)
                } else {
                    EvalResult::Value(self.null_ref)
                }
            }

            Expression::Index(index_expr) => {
                let left_ref = match self.eval_expression(&index_expr.left) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let idx_ref = match self.eval_expression(&index_expr.index) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };

                let idx_data = self.resolve(idx_ref).unwrap().clone();
                // A DateField indexes as its integer value (e.g. arr[date.month]).
                let idx_data = match idx_data {
                    ObjectData::DateField { value, .. } => ObjectData::Integer(value),
                    other => other,
                };

                // Dict fast path: look up against the arena slot directly — no O(N)
                // clone of the whole dict per read, and the slot-resident hash index
                // stays warm across lookups (cloning a dict resets its index).
                if let Some(ObjectData::Dict { entries, index, .. }) = self.resolve(left_ref) {
                    let search_key = obj_data_to_key_str(&idx_data);
                    let found = index.lookup(entries, &search_key).map(|i| entries[i].1.clone());
                    return match found {
                        Some(v) => EvalResult::Value(self.plant_global(v)),
                        None => EvalResult::Value(self.null_ref),
                    };
                }

                let left_data = self.resolve(left_ref).unwrap().clone();
                match (&left_data, &idx_data) {
                    (ObjectData::Str(s), ObjectData::Integer(i)) => {
                        let chars: Vec<char> = s.chars().collect();
                        if *i < 0 || *i as usize >= chars.len() {
                            EvalResult::Value(self.null_ref)
                        } else {
                            let c = chars[*i as usize].to_string();
                            EvalResult::Value(self.alloc(ObjectData::Str(c)))
                        }
                    }
                    (ObjectData::Array { elements, .. }, ObjectData::Integer(i)) => {
                        if *i < 0 || *i as usize >= elements.len() {
                            let (i, len) = (*i, elements.len());
                            self.rt_err_kind("IndexOutOfBounds", format!("Index out of bounds: {} (length {})", i, len))
                        } else {
                            EvalResult::Value(self.plant(elements[*i as usize].clone()))
                        }
                    }
                    _ => {
                        let tn = left_data.type_name().to_string();
                        self.rt_err_kind("TypeError", format!("Index operator not supported for type '{}'", tn))
                    }
                }
            }

            Expression::DictLiteral(dict_lit) => {
                let mut entries: Vec<(OwnedValue, OwnedValue)> = Vec::new();
                for (key_expr, val_expr) in &dict_lit.entries {
                    let key_ref = match self.eval_expression(key_expr) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    let val_ref = match self.eval_expression(val_expr) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };

                    if dict_lit.key_type != "any" {
                        let kd = self.resolve(key_ref).unwrap();
                        let valid = type_matches(&dict_lit.key_type, kd);
                        if !valid {
                            eprintln!("❌ TYPE ERROR: Dict key does not match declared key type '{}'", dict_lit.key_type);
                            return EvalResult::Error;
                        }
                    }
                    if dict_lit.value_type != "any" {
                        let vd = self.resolve(val_ref).unwrap();
                        let valid = type_matches(&dict_lit.value_type, vd);
                        if !valid {
                            eprintln!("❌ TYPE ERROR: Dict value does not match declared value type '{}'", dict_lit.value_type);
                            return EvalResult::Error;
                        }
                    }

                    entries.push((self.extract(key_ref), self.extract(val_ref)));
                }
                EvalResult::Value(self.alloc(ObjectData::Dict {
                    key_type: dict_lit.key_type.clone(),
                    value_type: dict_lit.value_type.clone(),
                    entries,
                    index: Default::default(),
                }))
            }

            Expression::EntryLiteral(_, _) => {
                eprintln!("❌ ERROR: Entry literal {{k,v}} is only valid as an argument to a dict method");
                EvalResult::Error
            }

            Expression::DotCall(dot_call) => {
                // super.method(args) — dispatch to parent class method
                if let Expression::Identifier(ref name) = *dot_call.object {
                    if name == "super" {
                        return self.eval_super_method_call(dot_call);
                    }
                    // ── Namespace dispatch (Math / File / JSON) ───────────────
                    if name == "Math" {
                        return self.eval_math_namespace(dot_call);
                    }
                    if name == "Regex" {
                        return self.eval_regex_namespace(dot_call);
                    }
                    if name == "File" {
                        return self.eval_file_namespace(dot_call);
                    }
                    if name == "JSON" {
                        return self.eval_json_namespace(dot_call);
                    }
                    if name == "Tensor" {
                        return self.eval_tensor_static(dot_call);
                    }
                    if name == "Crypto" {
                        return self.eval_crypto_namespace(dot_call);
                    }
                    if name == "Socket" {
                        return self.eval_socket_namespace(dot_call);
                    }
                    if name == "Binary" {
                        return self.eval_binary_namespace(dot_call);
                    }
                    if name == "GPU" {
                        return self.eval_gpu_namespace(dot_call);
                    }
                    if name == "Memory" {
                        return self.eval_memory_namespace(dot_call);
                    }
                    if name == "Random" {
                        return self.eval_random_namespace(dot_call);
                    }
                    if name == "Autodiff" {
                        return self.eval_autodiff_namespace(dot_call);
                    }
                    if name == "Terminal" {
                        return self.eval_terminal_namespace(dot_call);
                    }
                    if name == "OS" {
                        return self.eval_os_namespace(dot_call);
                    }
                    if name == "Env" {
                        return self.eval_env_namespace(dot_call);
                    }
                    if name == "Time" {
                        return self.eval_time_namespace(dot_call);
                    }
                    if name == "DateTime" {
                        return self.eval_datetime_namespace(dot_call);
                    }
                    if name == "Dec" {
                        return self.eval_dec_namespace(dot_call);
                    }
                    if name == "System" {
                        return self.eval_system_namespace(dot_call);
                    }
                    if name == "Gui" {
                        return self.eval_gui_namespace(dot_call);
                    }
                    if name == "Media" {
                        return self.eval_media_namespace(dot_call);
                    }
                    if name == "Task" {
                        return self.eval_task_namespace(dot_call);
                    }
                    // ── Enum variant access: Color.Red ────────────────────────
                    if let Some(variants) = self.enum_registry.get(name).cloned() {
                        let variant = dot_call.method.clone();
                        if variants.contains(&variant) {
                            return EvalResult::Value(self.alloc(ObjectData::EnumVariant {
                                enum_name: name.clone(),
                                variant,
                            }));
                        }
                        eprintln!("❌ ERROR: '{}' is not a variant of enum '{}'", dot_call.method, name);
                        return EvalResult::Error;
                    }
                    // ── Static method call: ClassName.method(args) ───────────────
                    if let Some(class) = self.class_registry.get(name).cloned() {
                        let method_name = dot_call.method.clone();
                        if let Some(m) = class.static_methods.get(&method_name).cloned() {
                            // Evaluate arguments
                            let mut arg_vals = Vec::new();
                            for arg in &dot_call.arguments {
                                match self.eval_expression(arg) {
                                    EvalResult::Value(v) => {
                                        let owned = self.extract(v);
                                        arg_vals.push(owned);
                                    }
                                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                                    _ => return EvalResult::Error,
                                }
                            }
                            // Create a temporary null instance ref for static dispatch
                            let fake_ref = self.null_ref;
                            return self.invoke_method(fake_ref, name, &m, arg_vals, 0, 0);
                        }
                    }
                }

                // Detect chained mutation pattern: instance.field.mutate(args)
                // After mutation we write the modified array/dict back to the instance field
                let writeback_ctx: Option<(Expression, String)> =
                    if let Expression::DotCall(inner) = dot_call.object.as_ref() {
                        if inner.arguments.is_empty() {
                            const MUTATING: &[&str] = &["push", "pop", "shift", "unshift", "sort", "remove", "reverse", "Add", "Remove", "RemoveAll", "clear"];
                            if MUTATING.contains(&dot_call.method.as_str()) {
                                Some((*inner.object.clone(), inner.method.clone()))
                            } else { None }
                        } else { None }
                    } else { None };

                let obj_ref = match self.eval_expression(&dot_call.object) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };

                // All Set methods run against the arena slot (methods_set.rs):
                // no O(N) clone of the receiver per call — even .size() paid
                // one — and the slot-resident hash index stays warm. Sets never
                // participated in the dict["key"] writeback (Array-only), but
                // `instance.field.remove/clear(...)` DOES write the mutated set
                // back into the field — same as the generic path below.
                if matches!(self.resolve(obj_ref), Some(ObjectData::Set { .. })) {
                    let result = self.eval_set_method_slot(obj_ref, dot_call);
                    if let Some((inner_obj_expr, field_name)) = writeback_ctx {
                        self.apply_field_writeback(&inner_obj_expr, &field_name, obj_ref);
                    }
                    return result;
                }

                // Array fast path for the loop builders/drainers: push/pop run
                // against the arena slot instead of cloning the whole array per
                // call (the generic path below made `a.push(x)` O(N) — building
                // an array in a loop was O(N²) in time). Index receivers
                // (`d["k"].push`, `m[i].push`) keep the generic path: they need
                // the dict-writeback machinery below.
                if matches!(dot_call.method.as_str(), "push" | "pop")
                    && !matches!(dot_call.object.as_ref(), Expression::Index(_))
                    && matches!(self.resolve(obj_ref), Some(ObjectData::Array { .. }))
                {
                    let result = self.eval_array_fast(obj_ref, dot_call);
                    if let Some((inner_obj_expr, field_name)) = writeback_ctx {
                        self.apply_field_writeback(&inner_obj_expr, &field_name, obj_ref);
                    }
                    return result;
                }

                let obj_data = match self.resolve(obj_ref) {
                    Some(d) => d.clone(),
                    None => {
                        eprintln!("❌ ERROR: Invalid reference in dot call");
                        return EvalResult::Error;
                    }
                };

                // Optional chaining: return null if object is null
                if dot_call.is_optional {
                    if let ObjectData::Null = obj_data {
                        return EvalResult::Value(self.null_ref);
                    }
                }

                // Detect dict["key"].mutatingMethod() pattern for writeback
                let dict_writeback_ctx: Option<(ObjectRef, String)> = {
                    const MUTATING_OPS: &[&str] = &["push","pop","shift","unshift","sort","remove","reverse","Add","Remove","RemoveAll","clear"];
                    if MUTATING_OPS.contains(&dot_call.method.as_str()) {
                        if let Expression::Index(idx_expr) = dot_call.object.as_ref() {
                            let (dict_expr, key_expr) = (idx_expr.left.as_ref(), idx_expr.index.as_ref());
                            if let Expression::Identifier(dict_name) = dict_expr {
                                if let Some(dr) = self.lookup_var(dict_name.as_str()) {
                                    if matches!(self.resolve(dr), Some(ObjectData::Dict { .. })) {
                                        let key_ref = match self.eval_expression(key_expr) {
                                            EvalResult::Value(r) => r,
                                            _ => return EvalResult::Error,
                                        };
                                        let key_str = match self.resolve(key_ref).cloned() {
                                            Some(ObjectData::Str(s)) => s,
                                            Some(ObjectData::Integer(n)) => n.to_string(),
                                            _ => String::new(),
                                        };
                                        if !key_str.is_empty() { Some((dr, key_str)) } else { None }
                                    } else { None }
                                } else { None }
                            } else { None }
                        } else { None }
                    } else { None }
                };

                let result = match obj_data {
                    // ── Array methods ─────────────────────────────────────────
                    ObjectData::Array { element_type, elements: ref elems } => {
                        let r = self.eval_array_method(obj_ref, element_type.clone(), elems.clone(), dot_call);
                        // Writeback: if array came from dict["key"], update the dict entry
                        if let Some((dict_ref, key_str)) = dict_writeback_ctx {
                            let updated = self.extract(obj_ref);
                            if let Some(ObjectData::Dict { key_type, value_type, mut entries, .. }) =
                                self.resolve(dict_ref).cloned()
                            {
                                let mut found = false;
                                for entry in entries.iter_mut() {
                                    let ks = match &entry.0 {
                                        OwnedValue::Str(s) => s.clone(),
                                        OwnedValue::Integer(n) => n.to_string(),
                                        _ => String::new(),
                                    };
                                    if ks == key_str { entry.1 = updated.clone(); found = true; break; }
                                }
                                if !found { entries.push((OwnedValue::Str(key_str), updated)); }
                                let new_dict = ObjectData::Dict { key_type, value_type, entries, index: Default::default() };
                                match dict_ref.region {
                                    crate::region::RegionId::Global => self.global_arena.update(dict_ref.index, new_dict),
                                    crate::region::RegionId::Scoped => self.scopes.arena.update(dict_ref.index, new_dict),
                                }
                            }
                        }
                        r
                    }

                    // ── String methods ────────────────────────────────────────
                    ObjectData::Str(ref s) => {
                        self.eval_string_method(s.clone(), dot_call)
                    }

                    // ── Dict methods ──────────────────────────────────────────
                    ObjectData::Dict { key_type, value_type, entries, .. } => {
                        let mut entries = entries;
                        match dot_call.method.as_str() {
                            "Add" => {
                                if dot_call.arguments.len() != 1 {
                                    eprintln!("❌ ERROR: Add expects 1 argument {{key, value}}");
                                    return EvalResult::Error;
                                }
                                let (key_ref, val_ref) = match &dot_call.arguments[0] {
                                    Expression::EntryLiteral(k_expr, v_expr) => {
                                        let k = match self.eval_expression(k_expr) {
                                            EvalResult::Value(r) => r,
                                            EvalResult::Throw(v) => return EvalResult::Throw(v),
                                            _ => return EvalResult::Error,
                                        };
                                        let v = match self.eval_expression(v_expr) {
                                            EvalResult::Value(r) => r,
                                            EvalResult::Throw(v) => return EvalResult::Throw(v),
                                            _ => return EvalResult::Error,
                                        };
                                        (k, v)
                                    }
                                    _ => {
                                        eprintln!("❌ ERROR: Add argument must be an entry literal {{key, value}}");
                                        return EvalResult::Error;
                                    }
                                };

                                if key_type != "any" {
                                    let kd = self.resolve(key_ref).unwrap();
                                    if !type_matches(&key_type, kd) {
                                        eprintln!("❌ TYPE ERROR: Dict key type mismatch on Add (expected '{}')", key_type);
                                        return EvalResult::Error;
                                    }
                                }
                                if value_type != "any" {
                                    let vd = self.resolve(val_ref).unwrap();
                                    if !type_matches(&value_type, vd) {
                                        eprintln!("❌ TYPE ERROR: Dict value type mismatch on Add (expected '{}')", value_type);
                                        return EvalResult::Error;
                                    }
                                }

                                let key_data = self.resolve(key_ref).unwrap().clone();
                                let search_key = obj_data_to_key_str(&key_data);

                                let mut replaced = false;
                                for (k, v) in entries.iter_mut() {
                                    if owned_to_key_str(k) == search_key {
                                        *v = self.extract(val_ref);
                                        replaced = true;
                                        break;
                                    }
                                }
                                if !replaced {
                                    let owned_k = self.extract(key_ref);
                                    let owned_v = self.extract(val_ref);
                                    entries.push((owned_k, owned_v));
                                }

                                self.update_dict(obj_ref, key_type, value_type, entries);
                                EvalResult::Value(self.null_ref)
                            }

                            "Remove" => {
                                if dot_call.arguments.len() != 1 {
                                    eprintln!("❌ ERROR: Remove expects 1 argument (key)");
                                    return EvalResult::Error;
                                }
                                let key_ref = match self.eval_expression(&dot_call.arguments[0]) {
                                    EvalResult::Value(r) => r,
                                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                                    _ => return EvalResult::Error,
                                };
                                let key_data = self.resolve(key_ref).unwrap().clone();
                                let search_key = obj_data_to_key_str(&key_data);

                                entries.retain(|(k, _)| {
                                    owned_to_key_str(k) != search_key
                                });

                                self.update_dict(obj_ref, key_type, value_type, entries);
                                EvalResult::Value(self.null_ref)
                            }

                            "RemoveAll" | "clear" => {
                                if !dot_call.arguments.is_empty() {
                                    eprintln!("❌ ERROR: {} expects no arguments", dot_call.method);
                                    return EvalResult::Error;
                                }
                                self.update_dict(obj_ref, key_type, value_type, Vec::new());
                                EvalResult::Value(self.null_ref)
                            }

                            // Returns array of keys: [k1, k2, ...]
                            "toList" | "keys" => {
                                let keys: Vec<OwnedValue> = entries.iter()
                                    .map(|(k, _)| k.clone())
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: keys }))
                            }

                            "values" => {
                                let vals: Vec<OwnedValue> = entries.iter()
                                    .map(|(_, v)| v.clone())
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: vals }))
                            }

                            // Returns 2-D array of entries: [[k1,v1],[k2,v2],...]
                            "toArray" => {
                                let pairs: Vec<OwnedValue> = entries.iter()
                                    .map(|(k, v)| OwnedValue::Array {
                                        element_type: None,
                                        elements: vec![k.clone(), v.clone()],
                                    })
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: pairs }))
                            }

                            "length" => {
                                EvalResult::Value(self.alloc(ObjectData::Integer(entries.len() as i64)))
                            }

                            "toString" => {
                                let s = self.display(obj_ref);
                                EvalResult::Value(self.alloc(ObjectData::Str(s)))
                            }

                            _ => {
                                eprintln!("❌ ERROR: Unknown dict method '{}'", dot_call.method);
                                EvalResult::Error
                            }
                        }
                    }
                    // ── Instance field read / method call ─────────────────────
                    ObjectData::Instance { class_name, fields } => {
                        self.eval_instance_dot(obj_ref, class_name, fields, dot_call)
                    }

                    // (Set methods are intercepted before this match — see the
                    // slot fast path above; a Set can never reach here.)

                    // ── Tensor methods ────────────────────────────────────────
                    ObjectData::Tensor { shape, data, .. } => {
                        self.eval_tensor_method(obj_ref, shape, data, dot_call)
                    }

                    // ── Exact decimal methods (round/setScale/abs/...) ─────────
                    ObjectData::Dec(d) => {
                        self.eval_dec_method(d, dot_call)
                    }

                    // ── DateTime field getters / methods ──────────────────────
                    ObjectData::DateTime { epoch_ms, utc } => {
                        self.eval_datetime_method(epoch_ms, utc, dot_call)
                    }

                    // ── DateField arithmetic (.add/.reduce/.remove) ───────────
                    ObjectData::DateField { epoch_ms, utc, field, value } => {
                        self.eval_datefield_method(epoch_ms, utc, field, value, dot_call)
                    }

                    // ── EnumVariant: no field access, just toString ────────────
                    ObjectData::EnumVariant { enum_name, variant } => {
                        if dot_call.method == "toString" {
                            let s = format!("{}.{}", enum_name, variant);
                            EvalResult::Value(self.alloc(ObjectData::Str(s)))
                        } else {
                            eprintln!("❌ ERROR: Enum variant has no method '{}'", dot_call.method);
                            EvalResult::Error
                        }
                    }

                    // .toString() available on all types
                    _ if dot_call.method == "toString" => {
                        let s = self.display(obj_ref);
                        EvalResult::Value(self.alloc(ObjectData::Str(s)))
                    }

                    _ => {
                        eprintln!("❌ ERROR: '.' method call not supported for type '{}'", obj_data.type_name());
                        EvalResult::Error
                    }
                };

                // Write back mutated array/dict to its instance field after mutation
                if let Some((inner_obj_expr, field_name)) = writeback_ctx {
                    self.apply_field_writeback(&inner_obj_expr, &field_name, obj_ref);
                }

                result
            }

            Expression::New(new_expr) => {
                // ── Built-in Tensor type ──────────────────────────────────────
                if new_expr.class_name == "Tensor" {
                    return self.eval_new_tensor(new_expr);
                }
                // ── Built-in Set type ─────────────────────────────────────────
                if new_expr.class_name == "Set" {
                    return self.eval_new_set(new_expr);
                }
                if let Some(iface) = self.interface_registry.get(&new_expr.class_name).cloned() {
                    return self.eval_new_interface(new_expr, iface);
                }
                if let Some(class) = self.class_registry.get(&new_expr.class_name).cloned() {
                    return self.eval_new_class(new_expr, class);
                }
                eprintln!("❌ ERROR: Unknown class or interface '{}'", new_expr.class_name);
                EvalResult::Error
            }

            Expression::ObjectPatch(_) => {
                eprintln!("❌ ERROR: Object patch '{{field: val}}' is only valid in an assignment context");
                EvalResult::Error
            }

            Expression::Ternary(ternary) => {
                let cond_ref = match self.eval_expression(&ternary.condition) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let cond_data = self.resolve(cond_ref).cloned().unwrap_or(ObjectData::Null);
                if self.is_truthy(&cond_data) {
                    self.eval_expression(&ternary.then_expr)
                } else {
                    self.eval_expression(&ternary.else_expr)
                }
            }

            Expression::Prefix(op, right_expr) => {
                let right_ref = match self.eval_expression(right_expr) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let right_data = self.resolve(right_ref).unwrap().clone();
                self.eval_prefix(op, right_ref, right_data)
            }

            // `expr is TypeName` — type check returning bool
            Expression::Infix(infix_expr) if infix_expr.operator == "is" => {
                let left_ref = match self.eval_expression(&infix_expr.left) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let left_data = self.resolve(left_ref).unwrap().clone();
                let type_name = match infix_expr.right.as_ref() {
                    Expression::Identifier(n) => n.as_str(),
                    _ => return EvalResult::Error,
                };
                let result = type_matches(type_name, &left_data);
                EvalResult::Value(self.bool_ref(result))
            }

            // Null coalescing: left ?? right — returns left if not null, else right
            Expression::Infix(infix_expr) if infix_expr.operator == "??" => {
                let left_ref = match self.eval_expression(&infix_expr.left) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                if !matches!(self.resolve(left_ref), Some(ObjectData::Null)) {
                    return EvalResult::Value(left_ref);
                }
                self.eval_expression(&infix_expr.right)
            }

            Expression::Infix(infix_expr)
                if infix_expr.operator == "&&" || infix_expr.operator == "||" =>
            {
                let left_ref = match self.eval_expression(&infix_expr.left) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let left_data = self.resolve(left_ref).unwrap().clone();
                let left_bool = match left_data {
                    ObjectData::Boolean(b) => b,
                    _ => {
                        eprintln!(
                            "❌ ERROR: '{}' operator requires boolean operands",
                            infix_expr.operator
                        );
                        return EvalResult::Error;
                    }
                };

                if infix_expr.operator == "&&" && !left_bool {
                    return EvalResult::Value(self.alloc(ObjectData::Boolean(false)));
                }
                if infix_expr.operator == "||" && left_bool {
                    return EvalResult::Value(self.alloc(ObjectData::Boolean(true)));
                }

                let right_ref = match self.eval_expression(&infix_expr.right) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                match self.resolve(right_ref).unwrap().clone() {
                    ObjectData::Boolean(_) => EvalResult::Value(right_ref),
                    _ => {
                        eprintln!(
                            "❌ ERROR: '{}' operator requires boolean operands",
                            infix_expr.operator
                        );
                        EvalResult::Error
                    }
                }
            }

            Expression::Infix(infix_expr) => {
                let left_ref = match self.eval_expression(&infix_expr.left) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let right_ref = match self.eval_expression(&infix_expr.right) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let left_data = self.resolve(left_ref).unwrap().clone();
                let right_data = self.resolve(right_ref).unwrap().clone();
                self.eval_infix(
                    &infix_expr.operator,
                    left_data,
                    right_data,
                    infix_expr.line,
                    infix_expr.column,
                )
            }

            // Spread used as a standalone expression — evaluate the inner value.
            // Actual spreading (into arrays/calls) is handled at the call/array site.
            Expression::Spread(inner) => self.eval_expression(inner),

            Expression::SizeOf(target) => {
                use crate::ast::SizeOfTarget;
                let size: i64 = match target {
                    SizeOfTarget::Type(name) => match name.as_str() {
                        "int"     => 8,
                        "decimal" => 8,
                        "bool"    => 1,
                        "string"  => 8,
                        "null"    => 0,
                        "void"    => 0,
                        "any"     => 8,
                        _         => 8, // unknown type: pointer-sized
                    },
                    SizeOfTarget::Expr(inner) => {
                        let val_ref = match self.eval_expression(inner) {
                            EvalResult::Value(r) => r,
                            other => return other,
                        };
                        match self.resolve(val_ref) {
                            Some(ObjectData::Integer(_))    => 8,
                            Some(ObjectData::Decimal(_))    => 8,
                            Some(ObjectData::Boolean(_))    => 1,
                            Some(ObjectData::Str(_))        => 8,
                            Some(ObjectData::Null)          => 0,
                            Some(ObjectData::Ptr(_))        => 8,
                            _                               => 8,
                        }
                    }
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(size)))
            }

            Expression::AddressOf(inner) => {
                if let Expression::Identifier(name) = inner.as_ref() {
                    if self.lookup_var(name).is_none() {
                        eprintln!("❌ ERROR: Cannot take address of undeclared variable '{}'", name);
                        return EvalResult::Error;
                    }
                    let ptr = ObjectData::Ptr(name.clone());
                    EvalResult::Value(self.alloc(ptr))
                } else {
                    eprintln!("❌ ERROR: '&' can only be applied to a named variable");
                    EvalResult::Error
                }
            }

            Expression::Deref(ptr_expr) => {
                let ptr_ref = match self.eval_expression(ptr_expr) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                match self.resolve(ptr_ref).cloned() {
                    Some(ObjectData::Ptr(name)) => {
                        match self.lookup_var(&name) {
                            Some(r) => EvalResult::Value(r),
                            None => {
                                eprintln!("❌ ERROR: Dangling pointer to '{}'", name);
                                EvalResult::Error
                            }
                        }
                    }
                    _ => {
                        eprintln!("❌ ERROR: Cannot dereference a non-pointer value");
                        EvalResult::Error
                    }
                }
            }

            Expression::Match(m) => {
                let subject_ref = match self.eval_expression(&m.subject) {
                    EvalResult::Value(v) => v,
                    other => return other,
                };
                let subject_data = match self.resolve(subject_ref) {
                    Some(d) => d.clone(),
                    None => return EvalResult::Error,
                };

                let arms = m.arms.clone();
                for arm in &arms {
                    let mut bindings: Vec<(String, ObjectRef)> = Vec::new();
                    if !self.match_pattern(&arm.pattern, &subject_data, subject_ref, &mut bindings) {
                        continue;
                    }

                    // Push scope for bindings, guard, and body
                    self.scopes.push();
                    for (name, val_ref) in &bindings {
                        self.scopes.declare(name.clone(), *val_ref);
                    }

                    // Evaluate guard if present
                    if let Some(guard) = &arm.guard {
                        let guard = guard.clone();
                        let guard_ref = match self.eval_expression(&guard) {
                            EvalResult::Value(v) => v,
                            other => { self.scopes.pop(); return other; }
                        };
                        let truthy = {
                            let d = self.resolve(guard_ref).unwrap();
                            self.is_truthy(d)
                        };
                        if !truthy {
                            self.scopes.pop();
                            continue;
                        }
                    }

                    // Evaluate body statements
                    let mut result_ref = self.null_ref;
                    let mut early: Option<EvalResult> = None;
                    let body = arm.body.clone();
                    for s in &body.statements {
                        match self.eval_statement(s) {
                            EvalResult::Value(v) => result_ref = v,
                            other => { early = Some(other); break; }
                        }
                    }

                    let owned = self.extract(result_ref);
                    self.scopes.pop();

                    if let Some(r) = early { return r; }
                    return EvalResult::Value(self.plant(owned));
                }

                // No arm matched — null
                EvalResult::Value(self.null_ref)
            }

            Expression::UnsafeBlock(block) => {
                let block = block.clone();
                self.eval_unsafe_block(&block)
            }
        }
    }

    fn match_pattern(
        &mut self,
        pattern: &ast::MatchPattern,
        subject: &ObjectData,
        subject_ref: ObjectRef,
        bindings: &mut Vec<(String, ObjectRef)>,
    ) -> bool {
        match pattern {
            ast::MatchPattern::Wildcard => true,
            ast::MatchPattern::Binding(name) => {
                bindings.push((name.clone(), subject_ref));
                true
            }
            ast::MatchPattern::Literal(lit_expr) => {
                let lit_ref = match self.eval_expression(lit_expr) {
                    EvalResult::Value(v) => v,
                    _ => return false,
                };
                let lit_data = match self.resolve(lit_ref) {
                    Some(d) => d.clone(),
                    None => return false,
                };
                self.values_equal(subject, &lit_data)
            }
            ast::MatchPattern::Or(patterns) => {
                for p in patterns {
                    let mut temp = Vec::new();
                    if self.match_pattern(p, subject, subject_ref, &mut temp) {
                        bindings.extend(temp);
                        return true;
                    }
                }
                false
            }
        }
    }

}
