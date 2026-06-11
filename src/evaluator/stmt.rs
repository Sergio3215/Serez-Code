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
    pub(super) fn eval_statement(&mut self, stmt: &Statement) -> EvalResult {
        match stmt {
            Statement::Let(let_stmt) => {
                let val_ref = match self.eval_expression(&let_stmt.value) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };

                // Always allocate a fresh slot so the variable never aliases its
                // source (e.g. `let x = arr[0]` must not share the slot with arr[0],
                // or a later `x = new_val` would silently mutate the array element).
                // Cloning ObjectData::Tensor preserves the tid, so autodiff tracking
                // works automatically via the stable tid in ad_tensor_ids.
                let fresh_data = self.resolve(val_ref).unwrap().clone();
                let new_ref = self.alloc(fresh_data);
                let val_ref = new_ref;

                if self.scopes.is_empty() {
                    self.global_bindings.insert(let_stmt.name.clone(), val_ref);
                } else {
                    self.scopes.declare(let_stmt.name.clone(), val_ref);
                }
                if let_stmt.is_const {
                    self.const_names.insert(let_stmt.name.clone());
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::Assign(assign_stmt) => {
                if self.lookup_var(&assign_stmt.name).is_none() {
                    eprintln!("❌ ERROR: Undeclared variable: {}", assign_stmt.name);
                    return EvalResult::Error;
                }
                if self.const_names.contains(&assign_stmt.name) {
                    eprintln!("❌ ERROR: Cannot reassign const '{}'", assign_stmt.name);
                    return EvalResult::Error;
                }

                // Interface patch: person = { field: val, ... }
                if let Expression::ObjectPatch(patch_fields) = &assign_stmt.value {
                    return self.eval_object_patch(&assign_stmt.name, patch_fields.clone());
                }

                let val_ref = match self.eval_expression(&assign_stmt.value) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let new_data = self.resolve(val_ref).unwrap().clone();

                if let Some(r) = self.scopes.assign(&assign_stmt.name, new_data.clone()) {
                    if r.region == RegionId::Global {
                        self.global_arena.update(r.index, new_data);
                    }
                    return EvalResult::Value(r);
                }

                if let Some(&existing_ref) = self.global_bindings.get(&assign_stmt.name) {
                    self.global_arena.update(existing_ref.index, new_data);
                    return EvalResult::Value(existing_ref);
                }

                EvalResult::Error
            }

            Statement::FunctionDeclaration(func_decl) => {
                let captured = self.capture_env(true); // named fn: reference semantics
                let func_data = ObjectData::Function {
                    return_type: func_decl.function.return_type.clone(),
                    parameters: Rc::new(func_decl.function.parameters.clone()),
                    body: Rc::new(func_decl.function.body.clone()),
                    captured: Rc::new(captured),
                    is_generator: func_decl.function.is_generator,
                };
                let func_ref = self.alloc(func_data);

                if self.scopes.is_empty() {
                    self.global_bindings
                        .insert(func_decl.name.clone(), func_ref);
                } else {
                    self.scopes.declare(func_decl.name.clone(), func_ref);
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::NativeDeclaration(decl) => {
                self.native_fns.insert(decl.name.clone());
                EvalResult::Value(self.null_ref)
            }

            Statement::Import(path) => {
                self.eval_import(path)
            }

            Statement::Export(inner) => {
                self.eval_export(inner)
            }

            Statement::LetDestructureArray(d) => self.eval_let_destructure_array(d),
            Statement::LetDestructureDict(d)  => self.eval_let_destructure_dict(d),

            Statement::Yield(expr) => {
                let val_ref = match self.eval_expression(expr) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if self.yield_collector.is_some() {
                    let owned = self.extract(val_ref);
                    self.yield_collector.as_mut().unwrap().push(owned);
                    EvalResult::Value(self.null_ref)
                } else {
                    eprintln!("❌ ERROR: 'yield' used outside of a generator function (fn*)");
                    EvalResult::Error
                }
            }

            Statement::UsePermissions(perms) => {
                for p in perms {
                    self.permissions.insert(p.clone());
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::Block(block_stmt) => self.eval_block(block_stmt),

            Statement::Unsafe(block_stmt) => self.eval_unsafe_block(block_stmt),

            Statement::DerefAssign { ptr, value } => {
                if !self.in_unsafe_block {
                    eprintln!("❌ ERROR: Pointer write through '*ptr = val' requires an unsafe {{ }} block");
                    return EvalResult::Error;
                }
                let ptr_ref = match self.eval_expression(ptr) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let var_name = match self.resolve(ptr_ref).cloned() {
                    Some(ObjectData::Ptr(name)) => name,
                    _ => {
                        eprintln!("❌ ERROR: Left side of '*ptr = val' is not a pointer");
                        return EvalResult::Error;
                    }
                };
                let val_ref = match self.eval_expression(value) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let new_data = self.resolve(val_ref).unwrap().clone();
                if let Some(r) = self.scopes.assign(&var_name, new_data.clone()) {
                    if r.region == RegionId::Global {
                        self.global_arena.update(r.index, new_data);
                    }
                    return EvalResult::Value(r);
                }
                if let Some(&existing_ref) = self.global_bindings.get(&var_name) {
                    self.global_arena.update(existing_ref.index, new_data);
                    return EvalResult::Value(existing_ref);
                }
                eprintln!("❌ ERROR: Pointer target '{}' not found in scope", var_name);
                EvalResult::Error
            }

            Statement::While(while_stmt) => {
                loop {
                    // Guardar watermark antes de evaluar la condición para liberar el temporal
                    // inmediatamente después — evita que se acumule una allocación por iteración.
                    let cond_mark = if !self.scopes.is_empty() {
                        Some(self.scopes.arena.watermark())
                    } else {
                        None
                    };

                    // 1. Evaluate condition
                    let condition_ref = match self.eval_expression(&while_stmt.condition) {
                        EvalResult::Value(v) => v,
                        EvalResult::Error => return EvalResult::Error,
                        EvalResult::Return(v) => return EvalResult::Return(v),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };

                    let condition_data = self.resolve(condition_ref).unwrap().clone();

                    // Liberar el temporal de la condición antes de decidir si continuar
                    if let Some(mark) = cond_mark {
                        self.scopes.arena.reset_to(mark);
                    }

                    if !self.is_truthy(&condition_data) {
                        break;
                    }

                    // 2. Evaluate body — body value is discarded (while is a statement, not expression)
                    match self.eval_block_discard(&while_stmt.body) {
                        EvalResult::Value(_) => {}
                        EvalResult::Break    => break,
                        EvalResult::Continue => continue,
                        EvalResult::Return(v) => return EvalResult::Return(v),
                        EvalResult::Error => return EvalResult::Error,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        EvalResult::BreakLabel(ref l) if while_stmt.label.as_deref() == Some(l.as_str()) => break,
                        EvalResult::ContinueLabel(ref l) if while_stmt.label.as_deref() == Some(l.as_str()) => continue,
                        other => return other,
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::DoWhile(do_stmt) => {
                loop {
                    // Execute body first
                    match self.eval_block_discard(&do_stmt.body) {
                        EvalResult::Value(_) => {}
                        EvalResult::Break    => break,
                        EvalResult::Continue => {}
                        EvalResult::Return(v) => return EvalResult::Return(v),
                        EvalResult::Error => return EvalResult::Error,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        EvalResult::BreakLabel(ref l) if do_stmt.label.as_deref() == Some(l.as_str()) => break,
                        EvalResult::ContinueLabel(ref l) if do_stmt.label.as_deref() == Some(l.as_str()) => {}
                        other => return other,
                    }
                    // Then check condition
                    let cond_ref = match self.eval_expression(&do_stmt.condition) {
                        EvalResult::Value(v) => v,
                        EvalResult::Error => return EvalResult::Error,
                        EvalResult::Return(v) => return EvalResult::Return(v),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if !self.is_truthy(self.resolve(cond_ref).unwrap()) {
                        break;
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::For(for_stmt) => {
                // Push a dedicated scope so the init variable is local to the loop
                self.scopes.push();

                // Init: declare the loop variable
                let init_val = match self.eval_expression(&for_stmt.init.value) {
                    EvalResult::Value(v) => v,
                    EvalResult::Error => { self.scopes.pop(); return EvalResult::Error; }
                    EvalResult::Return(v) => {
                        let owned = self.extract(v);
                        self.scopes.pop();
                        return EvalResult::Return(self.plant(owned));
                    }
                    EvalResult::Throw(v) => {
                        let owned = self.extract(v);
                        self.scopes.pop();
                        return EvalResult::Throw(self.plant(owned));
                    }
                    _ => { self.scopes.pop(); return EvalResult::Error; }
                };
                // Fresh slot to prevent aliasing (e.g. `for (let i = arr[0]; ...)` would
                // otherwise share the slot with arr[0] and corrupt the array on update).
                let init_data = self.resolve(init_val).unwrap().clone();
                let fresh_init = self.alloc(init_data);
                self.scopes.declare(for_stmt.init.name.clone(), fresh_init);

                // loop_return / loop_throw hold extracted values if a return/throw was
                // encountered inside the body — extracted BEFORE the for-scope is popped.
                let mut loop_return: Option<OwnedValue> = None;
                let mut loop_throw:  Option<OwnedValue> = None;
                let mut loop_error = false;

                loop {
                    // Evaluate condition, free its temporary immediately
                    let cond_mark = self.scopes.arena.watermark();
                    let condition_ref = match self.eval_expression(&for_stmt.condition) {
                        EvalResult::Value(v) => v,
                        EvalResult::Error => { loop_error = true; break; }
                        EvalResult::Return(v) => { loop_return = Some(self.extract(v)); break; }
                        EvalResult::Throw(v)  => { loop_throw  = Some(self.extract(v)); break; }
                        _ => { loop_error = true; break; }
                    };
                    let condition_data = self.resolve(condition_ref).unwrap().clone();
                    self.scopes.arena.reset_to(cond_mark);

                    if !self.is_truthy(&condition_data) {
                        break;
                    }

                    // Execute body — eval_block_discard handles its own push/pop
                    match self.eval_block_discard(&for_stmt.body) {
                        EvalResult::Value(_) => {}
                        EvalResult::Break => break,
                        EvalResult::Continue => {} // fall through to update
                        EvalResult::Return(v) => {
                            loop_return = Some(self.extract(v));
                            break;
                        }
                        EvalResult::Throw(v) => {
                            loop_throw = Some(self.extract(v));
                            break;
                        }
                        EvalResult::Error => {
                            loop_error = true;
                            break;
                        }
                        EvalResult::BreakLabel(ref l) if for_stmt.label.as_deref() == Some(l.as_str()) => break,
                        EvalResult::ContinueLabel(ref l) if for_stmt.label.as_deref() == Some(l.as_str()) => {} // fall to update
                        other => {
                            // Propagate label signals upward
                            let owned_throw = if let EvalResult::Throw(v) = &other { Some(self.extract(*v)) } else { None };
                            let owned_ret = if let EvalResult::Return(v) = &other { Some(self.extract(*v)) } else { None };
                            self.scopes.pop();
                            if let Some(owned) = owned_throw { return EvalResult::Throw(self.plant(owned)); }
                            if let Some(owned) = owned_ret { return EvalResult::Return(self.plant(owned)); }
                            return other;
                        }
                    }

                    // Evaluate update, free its temporaries, then assign in-place
                    let update_mark = self.scopes.arena.watermark();
                    let new_val_ref = match self.eval_expression(&for_stmt.update.value) {
                        EvalResult::Value(v) => v,
                        _ => {
                            self.scopes.arena.reset_to(update_mark);
                            loop_error = true;
                            break;
                        }
                    };
                    let new_data = self.resolve(new_val_ref).unwrap().clone();
                    self.scopes.arena.reset_to(update_mark);

                    if let Some(r) = self.scopes.assign(&for_stmt.update.name, new_data.clone()) {
                        if r.region == RegionId::Global {
                            self.global_arena.update(r.index, new_data);
                        }
                    } else if let Some(&existing_ref) =
                        self.global_bindings.get(&for_stmt.update.name)
                    {
                        self.global_arena.update(existing_ref.index, new_data);
                    } else {
                        eprintln!(
                            "❌ ERROR: Undeclared variable in for-loop update: {}",
                            for_stmt.update.name
                        );
                        loop_error = true;
                        break;
                    }
                }

                // Pop the for-scope AFTER extracting any return/throw value above
                self.scopes.pop();

                if loop_error { return EvalResult::Error; }
                if let Some(owned) = loop_throw  { return EvalResult::Throw(self.plant(owned)); }
                if let Some(owned) = loop_return { return EvalResult::Return(self.plant(owned)); }
                EvalResult::Value(self.null_ref)
            }

            Statement::IndexAssign(stmt) => {
                let target = stmt.target.clone();
                let index = stmt.index.clone();
                let value = stmt.value.clone();

                // For DotCall targets (this.field[i] = val) we need to writeback after mutation.
                // A zero-arg DotCall with an Identifier object is a field access, not a method call.
                let writeback: Option<(String, String)> = match &target {
                    Expression::DotCall(dc) if dc.arguments.is_empty() => {
                        if let Expression::Identifier(obj_name) = dc.object.as_ref() {
                            Some((obj_name.clone(), dc.method.clone()))
                        } else {
                            None
                        }
                    }
                    _ => None,
                };

                // Resolve the target array
                let arr_ref = match &target {
                    Expression::Identifier(name) => match self.lookup_var(name) {
                        Some(r) => r,
                        None => {
                            eprintln!("❌ ERROR: Variable not found: {}", name);
                            return EvalResult::Error;
                        }
                    },
                    _ => match self.eval_expression(&target) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    },
                };

                // Re-bind stmt fields from clones so the existing code below can use them
                let stmt = ast::IndexAssignStatement { target, index, value };

                // Evaluate index and new value
                let idx_ref = match self.eval_expression(&stmt.index) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let val_ref = match self.eval_expression(&stmt.value) {
                    EvalResult::Value(v) => v,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };

                let arr_data = self.resolve(arr_ref).unwrap().clone();
                let idx_data = self.resolve(idx_ref).unwrap().clone();

                match arr_data {
                    ObjectData::Array { element_type, mut elements } => {
                        let i = match idx_data {
                            ObjectData::Integer(i) => i,
                            _ => {
                                eprintln!("❌ ERROR: Array index must be an integer");
                                return EvalResult::Error;
                            }
                        };

                        if i < 0 || i as usize >= elements.len() {
                            eprintln!("❌ ERROR: Index out of bounds");
                            return EvalResult::Error;
                        }

                        if let Some(ref et) = element_type {
                            let val_data = self.resolve(val_ref).unwrap();
                            if !type_matches(et, val_data) {
                                eprintln!(
                                    "❌ TYPE ERROR: Cannot assign '{}' to [{}] array element",
                                    val_data.type_name(), et
                                );
                                return EvalResult::Error;
                            }
                        }

                        let owned = self.extract(val_ref);
                        elements[i as usize] = owned;

                        match arr_ref.region {
                            RegionId::Global => {
                                self.global_arena.update(arr_ref.index, ObjectData::Array { element_type, elements });
                            }
                            RegionId::Scoped => {
                                self.scopes.arena.update(arr_ref.index, ObjectData::Array { element_type, elements });
                            }
                        }
                    }

                    ObjectData::Dict { key_type, value_type, mut entries } => {
                        {
                            let val_data = self.resolve(val_ref).unwrap();
                            if !type_matches(&value_type, val_data) {
                                eprintln!(
                                    "❌ TYPE ERROR: Cannot assign '{}' to <{},{}> dict value",
                                    val_data.type_name(), key_type, value_type
                                );
                                return EvalResult::Error;
                            }
                        }
                        let search_key = obj_data_to_key_str(&idx_data);
                        let owned_val = self.extract(val_ref);

                        let mut replaced = false;
                        for entry in entries.iter_mut() {
                            let key_str = match &entry.0 {
                                OwnedValue::Str(s) => s.clone(),
                                OwnedValue::Integer(i) => i.to_string(),
                                OwnedValue::Decimal(d) => d.to_string(),
                                OwnedValue::Boolean(b) => b.to_string(),
                                _ => format!("{:?}", entry.0),
                            };
                            if key_str == search_key {
                                entry.1 = owned_val.clone();
                                replaced = true;
                                break;
                            }
                        }
                        if !replaced {
                            entries.push((OwnedValue::Str(search_key), owned_val));
                        }
                        self.update_dict(arr_ref, key_type, value_type, entries);
                    }

                    _ => {
                        eprintln!("❌ ERROR: Target is not an array or dict");
                        return EvalResult::Error;
                    }
                }

                // Writeback: if target was this.field[i], update the instance field
                if let Some((obj_name, field_name)) = writeback {
                    let updated_owned = self.extract(arr_ref);
                    if let Some(obj_ref) = self.lookup_var(&obj_name) {
                        if let Some(ObjectData::Instance { class_name, mut fields }) =
                            self.resolve(obj_ref).map(|d| d.clone())
                        {
                            if let Some(entry) = fields.iter_mut().find(|(k, _)| k == &field_name) {
                                entry.1 = updated_owned;
                            }
                            let inst = ObjectData::Instance { class_name, fields };
                            match obj_ref.region {
                                RegionId::Global => self.global_arena.update(obj_ref.index, inst),
                                RegionId::Scoped => self.scopes.arena.update(obj_ref.index, inst),
                            }
                        }
                    }
                }

                EvalResult::Value(self.null_ref)
            }

            Statement::Return(return_stmt) => {
                match self.eval_expression(&return_stmt.return_value) {
                    EvalResult::Value(v) => EvalResult::Return(v),
                    EvalResult::Throw(v) => EvalResult::Throw(v),
                    _ => EvalResult::Error,
                }
            }

            Statement::Break               => EvalResult::Break,
            Statement::Continue            => EvalResult::Continue,
            Statement::BreakLabel(l)       => EvalResult::BreakLabel(l.clone()),
            Statement::ContinueLabel(l)    => EvalResult::ContinueLabel(l.clone()),

            Statement::Out(out_stmt) => match self.eval_expression(&out_stmt.value) {
                EvalResult::Value(v) => {
                    match self.fmt_value(v) {
                        Ok(s)  => println!("{}", s),
                        Err(e) => return e,
                    }
                    EvalResult::Value(self.null_ref)
                }
                EvalResult::Return(v) => EvalResult::Return(v),
                EvalResult::Throw(v)  => EvalResult::Throw(v),
                EvalResult::Error => EvalResult::Error,
                other => other,
            },

            Statement::Expression(expr) => self.eval_expression(expr),

            Statement::Throw(expr) => {
                let val = match self.eval_expression(expr) {
                    EvalResult::Value(v) => v,
                    other => return other,
                };
                EvalResult::Throw(val)
            }

            Statement::ForEach(fe) => self.eval_foreach(fe),

            Statement::Switch(sw) => self.eval_switch(sw),
            Statement::Try(try_stmt) => self.eval_try(try_stmt),

            Statement::InterfaceDeclaration(decl) => {
                self.interface_registry.insert(decl.name.clone(), decl.fields.clone());
                EvalResult::Value(self.null_ref)
            }

            Statement::ClassDeclaration(decl) => {
                // Check sealed inheritance
                if let Some(ref parent_name) = decl.parent {
                    if self.sealed_classes.contains(parent_name) {
                        eprintln!("❌ ERROR: Cannot inherit from sealed class '{}'", parent_name);
                        return EvalResult::Error;
                    }
                }
                if decl.is_sealed {
                    self.sealed_classes.insert(decl.name.clone());
                }
                let mut methods = HashMap::new();
                let mut static_methods = HashMap::new();
                let mut getters = HashMap::new();
                let mut setters = HashMap::new();
                for m in &decl.methods {
                    if m.is_getter {
                        getters.insert(m.name.clone(), m.clone());
                    } else if m.is_setter {
                        setters.insert(m.name.clone(), m.clone());
                    } else {
                        if m.is_static {
                            static_methods.insert(m.name.clone(), m.clone());
                        }
                        methods.insert(m.name.clone(), m.clone());
                    }
                }
                self.class_registry.insert(decl.name.clone(), StoredClass {
                    parent: decl.parent.clone(),
                    constructor: decl.constructor.clone(),
                    methods,
                    static_methods,
                    getters,
                    setters,
                    is_abstract: decl.is_abstract,
                    is_sealed: decl.is_sealed,
                    fields: decl.fields.clone(),
                });
                EvalResult::Value(self.null_ref)
            }

            Statement::EnumDeclaration(decl) => {
                self.enum_registry.insert(decl.name.clone(), decl.variants.clone());
                EvalResult::Value(self.null_ref)
            }

            Statement::FieldAssign(stmt) => {
                let val_ref = match self.eval_expression(&stmt.value) {
                    EvalResult::Value(r) => r,
                    other => return other,
                };
                let new_val = self.extract(val_ref);

                let obj_ref = match self.lookup_var(&stmt.object) {
                    Some(r) => r,
                    None => {
                        eprintln!("❌ ERROR: Undeclared variable '{}' in field assignment", stmt.object);
                        return EvalResult::Error;
                    }
                };

                if let Some(ObjectData::Instance { class_name, mut fields }) = self.resolve(obj_ref).cloned() {
                    // Check for setter first
                    if let Some(setter) = self.find_setter(&class_name, &stmt.field) {
                        return self.invoke_method(obj_ref, &class_name.clone(), &setter, vec![new_val], 0, 0);
                    }
                    // Getter exists but no setter → read-only property, cannot assign
                    if self.find_getter(&class_name, &stmt.field).is_some() {
                        eprintln!(
                            "❌ ERROR: '{}' is a getter-only property of '{}' (no setter defined)",
                            stmt.field, class_name
                        );
                        return EvalResult::Error;
                    }
                    if let Some(f) = fields.iter_mut().find(|(n, _)| n == &stmt.field) {
                        f.1 = new_val;
                    } else {
                        fields.push((stmt.field.clone(), new_val));
                    }
                    match obj_ref.region {
                        RegionId::Global => self.global_arena.update(obj_ref.index, ObjectData::Instance { class_name, fields }),
                        RegionId::Scoped => self.scopes.arena.update(obj_ref.index, ObjectData::Instance { class_name, fields }),
                    }
                    EvalResult::Value(self.null_ref)
                } else {
                    eprintln!("❌ ERROR: '{}' is not a class or interface instance", stmt.object);
                    EvalResult::Error
                }
            }
        }
    }

    pub(super) fn eval_foreach(&mut self, stmt: &ast::ForEachStatement) -> EvalResult {
        let iter_ref = match self.eval_expression(&stmt.iterable) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };

        let items: Vec<OwnedValue> = match self.resolve(iter_ref).cloned() {
            Some(ObjectData::Array { elements, .. }) => {
                elements.iter().cloned().collect()
            }
            Some(ObjectData::Str(s)) => {
                s.chars().map(|c| OwnedValue::Str(c.to_string())).collect()
            }
            Some(ObjectData::Dict { entries, .. }) => {
                entries.iter().map(|(k, _)| k.clone()).collect()
            }
            _ => {
                eprintln!("❌ ERROR: for-in requires an array, string, or dict");
                return EvalResult::Error;
            }
        };

        self.scopes.push();

        // Pre-declare bindings
        match &stmt.var {
            ast::ForEachVar::Name(n) => { self.scopes.declare(n.clone(), self.null_ref); }
            ast::ForEachVar::Array(slots, rest) => {
                for s in slots { if let Some(n) = s { self.scopes.declare(n.clone(), self.null_ref); } }
                if let Some(r) = rest { self.scopes.declare(r.clone(), self.null_ref); }
            }
        }

        let mut loop_return: Option<OwnedValue> = None;
        let mut loop_throw:  Option<OwnedValue> = None;
        let mut loop_error = false;

        for item in items {
            let item_ref = self.plant(item);

            match &stmt.var.clone() {
                ast::ForEachVar::Name(n) => {
                    self.scopes.declare(n.clone(), item_ref);
                }
                ast::ForEachVar::Array(slots, rest) => {
                    // item must be an array; destructure it
                    let elems: Vec<OwnedValue> = match self.resolve(item_ref).cloned() {
                        Some(ObjectData::Array { elements, .. }) => {
                            elements.iter().cloned().collect()
                        }
                        _ => {
                            eprintln!("❌ ERROR: for-in destructure expects each item to be an array");
                            loop_error = true;
                            break;
                        }
                    };
                    for (i, slot) in slots.iter().enumerate() {
                        if let Some(name) = slot {
                            let v = elems.get(i).cloned().unwrap_or(OwnedValue::Null);
                            let r = self.plant(v);
                            self.scopes.declare(name.clone(), r);
                        }
                    }
                    if let Some(rest_name) = rest {
                        let start = slots.len();
                        let rest_items: Vec<OwnedValue> = elems.into_iter().skip(start).collect();
                        let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_items });
                        self.scopes.declare(rest_name.clone(), rest_ref);
                    }
                }
            }

            match self.eval_block_discard(&stmt.body) {
                EvalResult::Value(_) => {}
                EvalResult::Break    => break,
                EvalResult::Continue => continue,
                EvalResult::Return(v) => { loop_return = Some(self.extract(v)); break; }
                EvalResult::Throw(v)  => { loop_throw  = Some(self.extract(v)); break; }
                EvalResult::Error     => { loop_error = true; break; }
                EvalResult::BreakLabel(ref l) if stmt.label.as_deref() == Some(l.as_str()) => break,
                EvalResult::ContinueLabel(ref l) if stmt.label.as_deref() == Some(l.as_str()) => continue,
                other => { loop_error = true; let _ = other; break; }
            }
        }

        self.scopes.pop();

        if let Some(owned) = loop_throw  { return EvalResult::Throw(self.plant(owned)); }
        if let Some(owned) = loop_return { return EvalResult::Return(self.plant(owned)); }
        if loop_error { return EvalResult::Error; }
        EvalResult::Value(self.null_ref)
    }

    fn eval_let_destructure_array(&mut self, d: &ast::LetDestructureArray) -> EvalResult {
        let val_ref = match self.eval_expression(&d.value) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };

        let elems: Vec<OwnedValue> = match self.resolve(val_ref).cloned() {
            Some(ObjectData::Array { elements, .. }) => {
                elements.iter().cloned().collect()
            }
            _ => {
                eprintln!("❌ ERROR: Array destructure requires an array on the right side");
                return EvalResult::Error;
            }
        };

        for (i, slot) in d.names.iter().enumerate() {
            if let Some(name) = slot {
                let v = elems.get(i).cloned().unwrap_or(OwnedValue::Null);
                let r = self.plant(v);
                if self.scopes.is_empty() {
                    self.global_bindings.insert(name.clone(), r);
                } else {
                    self.scopes.declare(name.clone(), r);
                }
                if d.is_const {
                    self.const_names.insert(name.clone());
                }
            }
        }

        if let Some(ref rest_name) = d.rest {
            let start = d.names.len();
            let rest_items: Vec<OwnedValue> = elems.into_iter().skip(start).collect();
            let rest_ref = self.alloc(ObjectData::Array { element_type: None, elements: rest_items });
            if self.scopes.is_empty() {
                self.global_bindings.insert(rest_name.clone(), rest_ref);
            } else {
                self.scopes.declare(rest_name.clone(), rest_ref);
            }
            if d.is_const {
                self.const_names.insert(rest_name.clone());
            }
        }

        EvalResult::Value(self.null_ref)
    }

    fn eval_let_destructure_dict(&mut self, d: &ast::LetDestructureDict) -> EvalResult {
        let val_ref = match self.eval_expression(&d.value) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };

        let entries: Vec<(OwnedValue, OwnedValue)> = match self.resolve(val_ref).cloned() {
            Some(ObjectData::Dict { entries, .. }) => {
                entries.iter().cloned().collect()
            }
            Some(ObjectData::Instance { fields, .. }) => {
                fields.iter().map(|(k, v)| (OwnedValue::Str(k.clone()), v.clone())).collect()
            }
            _ => {
                eprintln!("❌ ERROR: Dict destructure requires a dict or object on the right side");
                return EvalResult::Error;
            }
        };

        for (key, alias) in &d.fields {
            let local_name = alias.as_deref().unwrap_or(key.as_str());
            let value = entries.iter()
                .find(|(k, _)| matches!(k, OwnedValue::Str(s) if s == key))
                .map(|(_, v)| v.clone())
                .unwrap_or(OwnedValue::Null);
            let r = self.plant(value);
            if self.scopes.is_empty() {
                self.global_bindings.insert(local_name.to_string(), r);
            } else {
                self.scopes.declare(local_name.to_string(), r);
            }
            if d.is_const {
                self.const_names.insert(local_name.to_string());
            }
        }

        EvalResult::Value(self.null_ref)
    }

    pub(super) fn eval_unsafe_block(&mut self, block: &ast::BlockStatement) -> EvalResult {
        let prev = self.in_unsafe_block;
        self.in_unsafe_block = true;
        let result = self.eval_block(block);
        self.in_unsafe_block = prev;
        result
    }

    /// Variante de eval_block para CUERPOS DE LOOP (posición de statement).
    /// Todos los llamadores de un cuerpo de loop descartan el Value del bloque,
    /// así que aquí NO se extrae ni se replanta el valor del último statement.
    /// Sin esto, un loop cuyo último statement produce un compuesto (p.ej.
    /// `arr = arr.map(...)` o `arr.reverse()`) plantaba una copia COMPLETA del
    /// compuesto por iteración en el frame del loop, que solo se libera al
    /// salir → retención M·N en loops largos (residuo de la fuga #1; medido:
    /// 400 MB en 300 iteraciones sobre un array de 20k). Return/Throw sí
    /// escapan del loop y se preservan igual que en eval_block.
    pub(super) fn eval_block_discard(&mut self, block: &ast::BlockStatement) -> EvalResult {
        self.scopes.push();
        let mut result = EvalResult::Value(self.null_ref);

        for s in &block.statements {
            match self.eval_statement(s) {
                EvalResult::Value(_)            => {} // descartado: ningún caller de cuerpo de loop lo usa
                EvalResult::Return(v)           => { result = EvalResult::Return(v);           break; }
                EvalResult::Break               => { result = EvalResult::Break;               break; }
                EvalResult::Continue            => { result = EvalResult::Continue;             break; }
                EvalResult::BreakLabel(l)       => { result = EvalResult::BreakLabel(l);       break; }
                EvalResult::ContinueLabel(l)    => { result = EvalResult::ContinueLabel(l);    break; }
                EvalResult::Error               => { result = EvalResult::Error;               break; }
                EvalResult::Throw(v)            => { result = EvalResult::Throw(v);            break; }
            }
        }

        // Solo Return/Throw escapan del bloque con un valor que el caller usa.
        let owned = match &result {
            EvalResult::Return(v) | EvalResult::Throw(v) => Some(self.extract(*v)),
            _ => None,
        };

        self.scopes.pop();

        match owned {
            Some(val) => {
                let promoted = self.plant(val);
                match result {
                    EvalResult::Return(_) => EvalResult::Return(promoted),
                    EvalResult::Throw(_)  => EvalResult::Throw(promoted),
                    _ => unreachable!(),
                }
            }
            None => result,
        }
    }

    pub(super) fn eval_block(&mut self, block: &ast::BlockStatement) -> EvalResult {
        self.scopes.push();
        let mut result = EvalResult::Value(self.null_ref);

        for s in &block.statements {
            match self.eval_statement(s) {
                EvalResult::Value(v) => result = EvalResult::Value(v),
                EvalResult::Return(v)           => { result = EvalResult::Return(v);           break; }
                EvalResult::Break               => { result = EvalResult::Break;               break; }
                EvalResult::Continue            => { result = EvalResult::Continue;             break; }
                EvalResult::BreakLabel(l)       => { result = EvalResult::BreakLabel(l);       break; }
                EvalResult::ContinueLabel(l)    => { result = EvalResult::ContinueLabel(l);    break; }
                EvalResult::Error               => { result = EvalResult::Error;               break; }
                EvalResult::Throw(v)            => { result = EvalResult::Throw(v);            break; }
            }
        }

        // Deep-extract ANTES del pop: preserva elementos de arrays y valores anidados.
        let owned = match &result {
            EvalResult::Value(v) | EvalResult::Return(v) | EvalResult::Throw(v) => Some(self.extract(*v)),
            EvalResult::Break | EvalResult::Continue | EvalResult::Error
            | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => None,
        };

        self.scopes.pop();

        match owned {
            Some(val) => {
                let promoted = self.plant(val);
                match result {
                    EvalResult::Value(_)  => EvalResult::Value(promoted),
                    EvalResult::Return(_) => EvalResult::Return(promoted),
                    EvalResult::Throw(_)  => EvalResult::Throw(promoted),
                    _ => unreachable!(),
                }
            }
            None => result, // Break, Continue, BreakLabel, ContinueLabel, or Error — pass through as-is
        }
    }

    fn eval_import_url(&mut self, url: &str) -> EvalResult {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        // Cache directory: ~/.serez/packages/
        let cache_dir = dirs_or_temp().join("packages");
        let _ = std::fs::create_dir_all(&cache_dir);

        // Use a hash of the URL as the cache filename
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        let hash = hasher.finish();
        let cache_file = cache_dir.join(format!("{:016x}.sz", hash));

        // Check if we have a cached copy
        let source = if cache_file.exists() {
            match std::fs::read_to_string(&cache_file) {
                Ok(s) => s,
                Err(e) => {
                    let msg = format!("ModuleNotFound: Cannot read cached module '{}': {}", url, e);
                    eprintln!("❌ ERROR: {}", msg);
                    let msg_ref = self.alloc(ObjectData::Str(msg));
                    return EvalResult::Throw(msg_ref);
                }
            }
        } else {
            // Download the module
            let agent = ureq::AgentBuilder::new()
                .timeout(std::time::Duration::from_secs(15))
                .build();
            match agent.get(url).call() {
                Ok(resp) => {
                    match resp.into_string() {
                        Ok(s) => {
                            let _ = std::fs::write(&cache_file, &s);
                            s
                        }
                        Err(e) => {
                            let msg = format!("ModuleNotFound: Cannot read response from '{}': {}", url, e);
                            eprintln!("❌ ERROR: {}", msg);
                            let msg_ref = self.alloc(ObjectData::Str(msg));
                            return EvalResult::Throw(msg_ref);
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("ModuleNotFound: Cannot fetch '{}': {}", url, e);
                    eprintln!("❌ ERROR: {}", msg);
                    let msg_ref = self.alloc(ObjectData::Str(msg));
                    return EvalResult::Throw(msg_ref);
                }
            }
        };

        // Use the cache_file path as canonical ID to prevent re-imports
        if self.imported_files.contains(&cache_file) {
            return EvalResult::Value(self.null_ref);
        }
        self.imported_files.insert(cache_file.clone());

        let prev_dir = self.current_dir.clone();
        self.current_dir = Some(cache_file.parent().unwrap_or(std::path::Path::new(".")).to_path_buf());

        let before_globals:    HashSet<String> = self.global_bindings.keys().cloned().collect();
        let before_classes:    HashSet<String> = self.class_registry.keys().cloned().collect();
        let before_interfaces: HashSet<String> = self.interface_registry.keys().cloned().collect();
        let before_enums:      HashSet<String> = self.enum_registry.keys().cloned().collect();

        let prev_exports = self.current_module_exports.take();
        self.current_module_exports = Some(HashSet::new());

        let lexer = crate::lexer::Lexer::new(source.clone());
        let mut parser = crate::parser::Parser::new(lexer);
        let source_lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
        parser.set_source(source_lines);
        let program = parser.parse_program();
        let result = self.eval_program(&program);

        let exports = self.current_module_exports.take().unwrap_or_default();
        self.current_module_exports = prev_exports;
        self.current_dir = prev_dir;

        if result.is_none() { return EvalResult::Error; }

        if !exports.is_empty() {
            self.global_bindings.retain(|k, _| before_globals.contains(k) || exports.contains(k));
            self.class_registry.retain(|k, _| before_classes.contains(k) || exports.contains(k));
            self.interface_registry.retain(|k, _| before_interfaces.contains(k) || exports.contains(k));
            self.enum_registry.retain(|k, _| before_enums.contains(k) || exports.contains(k));
        }

        EvalResult::Value(self.null_ref)
    }

    fn eval_export(&mut self, inner: &crate::ast::Statement) -> EvalResult {
        // Evaluate the inner declaration normally
        let result = self.eval_statement(inner);

        // If we're inside a module being imported, register the exported name
        if self.current_module_exports.is_some() {
            if let Some(name) = declaration_name(inner) {
                if let Some(ref mut exports) = self.current_module_exports {
                    exports.insert(name);
                }
            }
        }

        result
    }

    fn eval_import(&mut self, path: &str) -> EvalResult {
        // URL imports — delegate to the package manager
        if path.starts_with("https://") || path.starts_with("http://") {
            return self.eval_import_url(path);
        }

        // Resolve .sz extension once
        let path_with_ext = if path.ends_with(".sz") {
            path.to_string()
        } else {
            format!("{}.sz", path)
        };

        // Candidate directories to search, in priority order:
        //  1. Current file's directory (relative import)
        //  2. Process working directory (project root)
        //  3. <cwd>/packages/ (local project packages — installed with `sz install`)
        //  4. SEREZ_HOME env var (installed stdlib / workspace root)
        //  5. Executable's directory (bundled stdlib)
        //  6. ~/.serez/packages/ (global fallback)
        let mut search_dirs: Vec<std::path::PathBuf> = Vec::new();

        if let Some(ref d) = self.current_dir {
            search_dirs.push(d.clone());
        }
        let cwd = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));
        search_dirs.push(cwd.clone());
        search_dirs.push(cwd.join("packages"));
        if let Ok(home) = std::env::var("SEREZ_HOME") {
            search_dirs.push(std::path::PathBuf::from(home));
        }
        if let Ok(exe) = std::env::current_exe() {
            if let Some(exe_dir) = exe.parent() {
                search_dirs.push(exe_dir.to_path_buf());
            }
        }
        search_dirs.push(crate::package_manager::packages_dir());

        // Try each candidate directory. For each base, also try <base>/<pkg>/index.sz
        // so that `import "pkg-name"` resolves to `<packages>/pkg-name/index.sz`.
        let canonical = search_dirs.iter()
            .flat_map(|base| {
                let direct  = base.join(&path_with_ext);
                let pkg_dir = base.join(path.trim_end_matches(".sz")).join("index.sz");
                vec![direct, pkg_dir]
            })
            .find_map(|p| if p.exists() { p.canonicalize().ok() } else { None });

        let canonical = match canonical {
            Some(c) => c,
            None => {
                let msg = format!("ModuleNotFound: Cannot find module '{}'", path);
                eprintln!("❌ ERROR: {}", msg);
                let msg_ref = self.alloc(ObjectData::Str(msg));
                return EvalResult::Throw(msg_ref);
            }
        };

        if self.imported_files.contains(&canonical) {
            return EvalResult::Value(self.null_ref); // already imported — skip
        }
        self.imported_files.insert(canonical.clone());

        let source = match std::fs::read_to_string(&canonical) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("❌ ERROR: Cannot read module '{}': {}", canonical.display(), e);
                return EvalResult::Error;
            }
        };

        // Save and update current_dir for nested imports
        let prev_dir = self.current_dir.clone();
        if let Some(parent) = canonical.parent() {
            self.current_dir = Some(parent.to_path_buf());
        }

        // Snapshot existing names before loading the module
        let before_globals:    HashSet<String> = self.global_bindings.keys().cloned().collect();
        let before_classes:    HashSet<String> = self.class_registry.keys().cloned().collect();
        let before_interfaces: HashSet<String> = self.interface_registry.keys().cloned().collect();
        let before_enums:      HashSet<String> = self.enum_registry.keys().cloned().collect();

        // Activate export tracking for this module
        let prev_exports = self.current_module_exports.take();
        self.current_module_exports = Some(HashSet::new());

        let lexer = crate::lexer::Lexer::new(source.clone());
        let mut parser = crate::parser::Parser::new(lexer);
        let source_lines: Vec<String> = source.lines().map(|l| l.to_string()).collect();
        parser.set_source(source_lines);
        let program = parser.parse_program();

        let result = self.eval_program(&program);

        // Collect the exports declared by this module
        let exports = self.current_module_exports.take().unwrap_or_default();
        self.current_module_exports = prev_exports;
        self.current_dir = prev_dir;

        if result.is_none() {
            return EvalResult::Error;
        }

        // If the module used `export`, enforce visibility: remove everything
        // that was added but NOT exported.
        if !exports.is_empty() {
            self.global_bindings.retain(|k, _| before_globals.contains(k) || exports.contains(k));
            self.class_registry.retain(|k, _| before_classes.contains(k) || exports.contains(k));
            self.interface_registry.retain(|k, _| before_interfaces.contains(k) || exports.contains(k));
            self.enum_registry.retain(|k, _| before_enums.contains(k) || exports.contains(k));
        }
        // If no `export` was used, everything the module defined stays (backwards compat)

        EvalResult::Value(self.null_ref)
    }

}

// Returns ~/.serez or a system temp fallback for package caching.
fn dirs_or_temp() -> std::path::PathBuf {
    if let Some(home) = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE")) {
        std::path::PathBuf::from(home).join(".serez")
    } else {
        std::env::temp_dir().join("serez")
    }
}

// Returns the declared name of a statement, if it has one.
fn declaration_name(stmt: &Statement) -> Option<String> {
    match stmt {
        Statement::Let(l)                   => Some(l.name.clone()),
        Statement::FunctionDeclaration(f)   => Some(f.name.clone()),
        Statement::ClassDeclaration(c)      => Some(c.name.clone()),
        Statement::InterfaceDeclaration(i)  => Some(i.name.clone()),
        Statement::EnumDeclaration(e)       => Some(e.name.clone()),
        Statement::NativeDeclaration(n)     => Some(n.name.clone()),
        _ => None,
    }
}
