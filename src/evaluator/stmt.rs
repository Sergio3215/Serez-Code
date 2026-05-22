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
                let fresh_data = self.resolve(val_ref).unwrap().clone();
                let val_ref = self.alloc(fresh_data);

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
                    parameters: func_decl.function.parameters.clone(),
                    body: Rc::new(func_decl.function.body.clone()),
                    captured,
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

            Statement::Block(block_stmt) => self.eval_block(block_stmt),

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
                    match self.eval_block(&while_stmt.body) {
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
                    match self.eval_block(&do_stmt.body) {
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
                    let cond_data = self.resolve(cond_ref).unwrap().clone();
                    if !self.is_truthy(&cond_data) {
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

                    // Execute body — eval_block handles its own push/pop
                    match self.eval_block(&for_stmt.body) {
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
                        let new_elem_ref = match arr_ref.region {
                            RegionId::Global => self.plant_global(owned),
                            RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(owned),
                            RegionId::Scoped => self.plant(owned),
                        };
                        elements[i as usize] = new_elem_ref;

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

                        // Use global arena when mutating a scoped dict from a nested block —
                        // values allocated in the inner scope are freed when that block exits.
                        let use_global = arr_ref.region == RegionId::Global
                            || self.scopes.depth() > 1;

                        // Index-based loop so we can call &mut self methods (plant/plant_global)
                        let mut replaced = false;
                        let mut i = 0;
                        while i < entries.len() {
                            let k_data = self.resolve(entries[i].0).unwrap().clone();
                            if obj_data_to_key_str(&k_data) == search_key {
                                let new_ref = if use_global {
                                    self.plant_global(owned_val.clone())
                                } else {
                                    self.plant(owned_val.clone())
                                };
                                entries[i].1 = new_ref;
                                replaced = true;
                                break;
                            }
                            i += 1;
                        }
                        if !replaced {
                            let owned_k = OwnedValue::Str(search_key);
                            let (new_k, new_v) = if use_global {
                                (self.plant_global(owned_k), self.plant_global(owned_val))
                            } else {
                                (self.plant(owned_k), self.plant(owned_val))
                            };
                            entries.push((new_k, new_v));
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
                self.class_registry.insert(decl.name.clone(), StoredClass {
                    parent: decl.parent.clone(),
                    constructor: decl.constructor.clone(),
                    methods: decl.methods.clone(),
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
                elements.iter().map(|&r| self.extract(r)).collect()
            }
            Some(ObjectData::Str(s)) => {
                s.chars().map(|c| OwnedValue::Str(c.to_string())).collect()
            }
            Some(ObjectData::Dict { entries, .. }) => {
                entries.iter().map(|(k, _)| self.extract(*k)).collect()
            }
            _ => {
                eprintln!("❌ ERROR: for-in requires an array, string, or dict");
                return EvalResult::Error;
            }
        };

        self.scopes.push();
        self.scopes.declare(stmt.var_name.clone(), self.null_ref);

        let mut loop_return: Option<OwnedValue> = None;
        let mut loop_throw:  Option<OwnedValue> = None;
        let mut loop_error = false;

        for item in items {
            let item_ref = self.plant(item);
            self.scopes.declare(stmt.var_name.clone(), item_ref);

            match self.eval_block(&stmt.body) {
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

}
