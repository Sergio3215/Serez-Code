use crate::ast::{self, Expression, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::HashMap;
use std::io::{self, Write};
use std::rc::Rc;

#[derive(Clone)]
struct StoredMethod {
    name: String,
    is_public: bool,
    return_type: Option<String>,
    parameters: Vec<ast::Parameter>,
    body: Rc<ast::BlockStatement>, // Rc: find_method clones this on every dispatch → O(1)
}

#[derive(Clone)]
struct StoredClass {
    parent: Option<String>,
    constructor: Option<ast::ClassConstructor>,
    methods: Vec<StoredMethod>,
}

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),  // Ejecución normal (retorno implícito)
    Return(ObjectRef), // Ejecución interrumpida por `return`
    Break,             // Señal de break — capturada por while/for
    Continue,          // Señal de continue — capturada por while/for
    Error,             // Ocurrió un error
    Throw(ObjectRef),  // Excepción de usuario — propagada hasta try/catch
}

// ── Evaluator ─────────────────────────────────────────────────────────────────
struct CallFrame {
    name: String,
    line: usize,
    column: usize,
}
pub struct Evaluator {
    global_arena: Arena,
    global_bindings: HashMap<String, ObjectRef>,
    scopes: ScopeStack,
    null_ref: ObjectRef,
    call_stack: Vec<CallFrame>,
    interface_registry: HashMap<String, Vec<ast::InterfaceField>>,
    class_registry: HashMap<String, StoredClass>,
    constructing_class: Option<String>,
    executing_class: Option<String>,
    call_depth: usize,
}

impl Evaluator {
    pub fn new() -> Self {
        let mut global_arena = Arena::with_capacity(256);
        let null_idx = global_arena.alloc(ObjectData::Null);
        let null_ref = ObjectRef {
            region: RegionId::Global,
            index: null_idx,
        };

        Evaluator {
            global_arena,
            global_bindings: HashMap::with_capacity(32),
            scopes: ScopeStack::new(),
            null_ref,
            call_stack: Vec::new(),
            interface_registry: HashMap::with_capacity(8),
            class_registry: HashMap::with_capacity(8),
            constructing_class: None,
            executing_class: None,
            call_depth: 0,
        }
    }

    fn alloc(&mut self, data: ObjectData) -> ObjectRef {
        if self.scopes.is_empty() {
            let idx = self.global_arena.alloc(data);
            ObjectRef {
                region: RegionId::Global,
                index: idx,
            }
        } else {
            let idx = self.scopes.arena.alloc(data);
            ObjectRef {
                region: RegionId::Scoped,
                index: idx,
            }
        }
    }

    pub fn resolve(&self, obj_ref: ObjectRef) -> Option<&ObjectData> {
        match obj_ref.region {
            RegionId::Global => self.global_arena.get(obj_ref.index),
            RegionId::Scoped => self.scopes.arena.get(obj_ref.index),
        }
    }

    fn lookup_var(&self, name: &str) -> Option<ObjectRef> {
        if let Some(r) = self.scopes.lookup(name) {
            return Some(r);
        }
        self.global_bindings.get(name).copied()
    }

    /// Captures the current lexical environment as global-arena ObjectRefs.
    /// Each captured variable is promoted to the global arena so that mutations
    /// inside the closure persist across calls (B-27 fix).
    /// Returns an empty vec at global scope (nothing to capture).
    fn capture_env(&mut self) -> Vec<(String, ObjectRef)> {
        let bindings = self.scopes.all_bindings();
        let mut result = Vec::with_capacity(bindings.len());
        for (name, r) in bindings {
            let global_ref = match r.region {
                RegionId::Global => r,
                RegionId::Scoped => {
                    let owned = self.extract(r);
                    self.plant_global(owned)
                }
            };
            result.push((name, global_ref));
        }
        result
    }

    pub fn display(&self, obj_ref: ObjectRef) -> String {
        match self.resolve(obj_ref) {
            Some(ObjectData::Integer(i)) => format!("{}", i),
            Some(ObjectData::Decimal(d)) => {
                if d.fract() == 0.0 {
                    format!("{:.1}", d)
                } else {
                    // 10 significant decimal places, trailing zeros trimmed
                    let s = format!("{:.10}", d);
                    s.trim_end_matches('0').trim_end_matches('.').to_string()
                }
            }
            Some(ObjectData::Boolean(b)) => format!("{}", b),
            Some(ObjectData::Str(s)) => s.to_string(),
            Some(ObjectData::Array { elements: refs, .. }) => {
                let elems: Vec<String> = refs.iter().map(|&r| self.display(r)).collect();
                format!("[{}]", elems.join(", "))
            }
            Some(ObjectData::Dict { entries, .. }) => {
                let entries = entries.clone();
                let pairs: Vec<String> = entries
                    .iter()
                    .map(|&(k, v)| format!("{}: {}", self.display(k), self.display(v)))
                    .collect();
                format!("{{{}}}", pairs.join(", "))
            }
            Some(ObjectData::Function { .. }) => "Function".to_string(),
            Some(ObjectData::Instance { class_name, fields }) => {
                let pairs: Vec<String> = fields.iter()
                    .map(|(n, v)| format!("{}: {}", n, v.display_str()))
                    .collect();
                format!("{}{{ {} }}", class_name, pairs.join(", "))
            }
            Some(ObjectData::Null) => "null".to_string(),
            None => "❌ Referencia inválida".to_string(),
        }
    }

    // Extrae un valor completo de la arena a un OwnedValue independiente.
    // Debe llamarse ANTES de scopes.pop() para que los índices aún sean válidos.
    fn extract(&self, obj_ref: ObjectRef) -> OwnedValue {
        match self.resolve(obj_ref) {
            Some(ObjectData::Integer(i)) => OwnedValue::Integer(*i),
            Some(ObjectData::Decimal(d)) => OwnedValue::Decimal(*d),
            Some(ObjectData::Boolean(b)) => OwnedValue::Boolean(*b),
            Some(ObjectData::Str(s)) => OwnedValue::Str(s.clone()),
            Some(ObjectData::Array { element_type, elements: refs }) => {
                OwnedValue::Array {
                    element_type: element_type.clone(),
                    elements: refs.iter().map(|&r| self.extract(r)).collect(),
                }
            }
            Some(ObjectData::Dict { key_type, value_type, entries }) => OwnedValue::Dict {
                key_type: key_type.clone(),
                value_type: value_type.clone(),
                entries: entries.iter().map(|&(k, v)| (self.extract(k), self.extract(v))).collect(),
            },
            Some(ObjectData::Function {
                return_type,
                parameters,
                body,
                captured,
            }) => OwnedValue::Function {
                return_type: return_type.clone(),
                parameters: parameters.clone(),
                body: body.clone(),
                captured: captured.clone(), // Vec<(String, ObjectRef)> — global refs are stable
            },
            Some(ObjectData::Instance { class_name, fields }) => OwnedValue::Instance {
                class_name: class_name.clone(),
                fields: fields.clone(),
            },
            Some(ObjectData::Null) | None => OwnedValue::Null,
        }
    }

    // Re-aloca un OwnedValue en la arena activa (scope padre o global).
    // Debe llamarse DESPUÉS de scopes.pop().
    fn plant(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => self.alloc(ObjectData::Integer(i)),
            OwnedValue::Decimal(d) => self.alloc(ObjectData::Decimal(d)),
            OwnedValue::Boolean(b) => self.alloc(ObjectData::Boolean(b)),
            OwnedValue::Str(s) => self.alloc(ObjectData::Str(s)),
            OwnedValue::Array { element_type, elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant(v)).collect();
                self.alloc(ObjectData::Array { element_type, elements: refs })
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                let planted: Vec<(ObjectRef, ObjectRef)> = entries
                    .into_iter()
                    .map(|(k, v)| (self.plant(k), self.plant(v)))
                    .collect();
                self.alloc(ObjectData::Dict { key_type, value_type, entries: planted })
            }
            OwnedValue::Function {
                return_type,
                parameters,
                body,
                captured,
            } => self.alloc(ObjectData::Function {
                return_type,
                parameters,
                body,
                captured,
            }),
            OwnedValue::Instance { class_name, fields } => {
                self.alloc(ObjectData::Instance { class_name, fields })
            }
            OwnedValue::Null => self.null_ref,
        }
    }

    // Igual que plant() pero siempre aloca en la arena global.
    // Necesario cuando se muta un array global desde dentro de un scope:
    // los nuevos elementos deben vivir en la misma arena que el array.
    fn plant_global(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => {
                let idx = self.global_arena.alloc(ObjectData::Integer(i));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Decimal(d) => {
                let idx = self.global_arena.alloc(ObjectData::Decimal(d));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Boolean(b) => {
                let idx = self.global_arena.alloc(ObjectData::Boolean(b));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Str(s) => {
                let idx = self.global_arena.alloc(ObjectData::Str(s));
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Array { element_type, elements: items } => {
                let refs: Vec<ObjectRef> =
                    items.into_iter().map(|v| self.plant_global(v)).collect();
                let idx = self.global_arena.alloc(ObjectData::Array { element_type, elements: refs });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Dict { key_type, value_type, entries } => {
                let planted: Vec<(ObjectRef, ObjectRef)> = entries
                    .into_iter()
                    .map(|(k, v)| (self.plant_global(k), self.plant_global(v)))
                    .collect();
                let idx = self.global_arena.alloc(ObjectData::Dict {
                    key_type,
                    value_type,
                    entries: planted,
                });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Function { return_type, parameters, body, captured } => {
                let idx = self.global_arena.alloc(ObjectData::Function {
                    return_type,
                    parameters,
                    body,
                    captured,
                });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Instance { class_name, fields } => {
                let idx = self.global_arena.alloc(ObjectData::Instance { class_name, fields });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Null => self.null_ref,
        }
    }

    // ── Evaluación de Programa ──────────────────────────────────────────────

    pub fn eval_program(&mut self, program: &Program) -> Option<ObjectRef> {
        let mut result = self.null_ref;
        for statement in &program.statements {
            // Out statements at top level produce values that are immediately consumed
            // (printed) and never retained. Use a scratch watermark so display
            // temporaries don't accumulate in the global arena for the program lifetime.
            //
            // NOTE: Expression statements (e.g. function calls used as statements) are
            // intentionally excluded here. A function call may have persistent side effects
            // such as IndexAssign on a global array: those allocations land in the global
            // arena and must survive the statement boundary. Resetting to a pre-call
            // watermark would free them, producing dangling refs.
            let scratch_mark = match statement {
                Statement::Out(_) => Some(self.global_arena.watermark()),
                _ => None,
            };

            match self.eval_statement(statement) {
                EvalResult::Value(v) => {
                    if scratch_mark.is_none() {
                        result = v;
                    }
                    if let Some(mark) = scratch_mark {
                        self.global_arena.reset_to(mark);
                    }
                }
                EvalResult::Return(_) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'return' cannot be used outside of a function or conditional or loops.");
                    return None;
                }
                EvalResult::Break => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'break' cannot be used outside of a loop.");
                    return None;
                }
                EvalResult::Continue => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'continue' cannot be used outside of a loop.");
                    return None;
                }
                EvalResult::Error => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    return None;
                }
                EvalResult::Throw(r) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    let msg = self.display(r);
                    eprintln!("❌ UNCAUGHT EXCEPTION: {msg}");
                    return None;
                }
            }
        }
        Some(result)
    }

    fn eval_statement(&mut self, stmt: &Statement) -> EvalResult {
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
                EvalResult::Value(self.null_ref)
            }

            Statement::Assign(assign_stmt) => {
                if self.lookup_var(&assign_stmt.name).is_none() {
                    eprintln!("❌ ERROR: Undeclared variable: {}", assign_stmt.name);
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
                let captured = self.capture_env();
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

                        // Index-based loop so we can call &mut self methods (plant_for_target)
                        let mut replaced = false;
                        let mut i = 0;
                        while i < entries.len() {
                            let k_data = self.resolve(entries[i].0).unwrap().clone();
                            if obj_data_to_key_str(&k_data) == search_key {
                                entries[i].1 = self.plant_for_target(owned_val.clone(), arr_ref);
                                replaced = true;
                                break;
                            }
                            i += 1;
                        }
                        if !replaced {
                            let owned_k = OwnedValue::Str(search_key);
                            let new_k = self.plant_for_target(owned_k, arr_ref);
                            let new_v = self.plant_for_target(owned_val, arr_ref);
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
                            self.resolve(obj_ref).cloned()
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

            Statement::Break    => EvalResult::Break,
            Statement::Continue => EvalResult::Continue,

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
                self.class_registry.insert(decl.name.clone(), StoredClass {
                    parent: decl.parent.clone(),
                    constructor: decl.constructor.clone(),
                    methods: decl.methods.iter().map(|m| StoredMethod {
                        name: m.name.clone(),
                        is_public: m.is_public,
                        return_type: m.return_type.clone(),
                        parameters: m.parameters.clone(),
                        body: Rc::new(m.body.clone()),
                    }).collect(),
                });
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

    fn eval_foreach(&mut self, stmt: &ast::ForEachStatement) -> EvalResult {
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
            }
        }

        self.scopes.pop();

        if let Some(owned) = loop_throw  { return EvalResult::Throw(self.plant(owned)); }
        if let Some(owned) = loop_return { return EvalResult::Return(self.plant(owned)); }
        if loop_error { return EvalResult::Error; }
        EvalResult::Value(self.null_ref)
    }

    fn eval_block(&mut self, block: &ast::BlockStatement) -> EvalResult {
        self.scopes.push();
        let mut result = EvalResult::Value(self.null_ref);

        for s in &block.statements {
            match self.eval_statement(s) {
                EvalResult::Value(v) => result = EvalResult::Value(v),
                EvalResult::Return(v)  => { result = EvalResult::Return(v);  break; }
                EvalResult::Break      => { result = EvalResult::Break;       break; }
                EvalResult::Continue   => { result = EvalResult::Continue;    break; }
                EvalResult::Error      => { result = EvalResult::Error;       break; }
                EvalResult::Throw(v)   => { result = EvalResult::Throw(v);   break; }
            }
        }

        // Deep-extract ANTES del pop: preserva elementos de arrays y valores anidados.
        let owned = match &result {
            EvalResult::Value(v) | EvalResult::Return(v) | EvalResult::Throw(v) => Some(self.extract(*v)),
            EvalResult::Break | EvalResult::Continue | EvalResult::Error => None,
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
            None => result, // Break, Continue, or Error — pass through as-is
        }
    }

    fn eval_expression(&mut self, expr: &Expression) -> EvalResult {
        match expr {
            Expression::Integer(i) => EvalResult::Value(self.alloc(ObjectData::Integer(*i))),
            Expression::Decimal(d) => EvalResult::Value(self.alloc(ObjectData::Decimal(*d))),
            Expression::String(s) => EvalResult::Value(self.alloc(ObjectData::Str(s.clone()))),
            Expression::Boolean(b) => EvalResult::Value(self.alloc(ObjectData::Boolean(*b))),
            Expression::Null => EvalResult::Value(self.null_ref),

            Expression::Identifier(name) => match self.lookup_var(name) {
                Some(r) => EvalResult::Value(r),
                None => {
                    eprintln!("❌ ERROR: Variable not found: {}", name);
                    EvalResult::Error
                }
            },

            Expression::FunctionLiteral(func_lit) => {
                let captured = self.capture_env();
                let func_data = ObjectData::Function {
                    return_type: func_lit.return_type.clone(),
                    parameters: func_lit.parameters.clone(),
                    body: Rc::new(func_lit.body.clone()),
                    captured,
                };
                EvalResult::Value(self.alloc(func_data))
            }

            Expression::Lambda(lambda) => {
                use crate::ast::{LambdaBody, Parameter, BlockStatement, Statement, ReturnStatement};
                let params: Vec<Parameter> = lambda.params.iter()
                    .map(|n| Parameter { name: n.clone(), type_name: None })
                    .collect();
                let body = match &lambda.body {
                    LambdaBody::Block(b) => b.clone(),
                    LambdaBody::Expr(e) => BlockStatement {
                        statements: vec![Statement::Return(ReturnStatement {
                            return_value: *e.clone(),
                        })],
                    },
                };
                let captured = self.capture_env();
                EvalResult::Value(self.alloc(ObjectData::Function {
                    return_type: None,
                    parameters: params,
                    body: Rc::new(body),
                    captured,
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
                        "super"       => return self.eval_super_call(&call_expr.arguments),
                        "assert"      => return self.eval_assert(&call_expr.arguments),
                        "type_of"     => return self.eval_type_of(&call_expr.arguments),
                        "abs" | "sqrt" | "floor" | "ceil" | "round"
                        | "min" | "max" | "pow" | "log" | "log2" | "log10"
                            => return self.eval_math_builtin(name, &call_expr.arguments),
                        _ => {}
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

                let func_data = self.resolve(func_ref).cloned();
                let (return_type, parameters, body, captured) = match func_data {
                    Some(ObjectData::Function {
                        return_type,
                        parameters,
                        body,
                        captured,
                    }) => (return_type, parameters, body, captured),
                    _ => {
                        eprintln!("❌ ERROR: Attempt to call a non-function");
                        self.leave_call();
                        return EvalResult::Error;
                    }
                };

                let mut arg_refs = Vec::new();
                for arg in &call_expr.arguments {
                    match self.eval_expression(arg) {
                        EvalResult::Value(r) => arg_refs.push(r),
                        _ => {
                            self.leave_call();
                            return EvalResult::Error;
                        }
                    }
                }

                if arg_refs.len() != parameters.len() {
                    eprintln!(
                        "❌ ERROR: Function expected {} arguments, got {}",
                        parameters.len(),
                        arg_refs.len()
                    );

                    self.print_call_stack();
                    self.leave_call();
                    return EvalResult::Error;
                }

                for (i, param) in parameters.iter().enumerate() {
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
                            self.leave_call();
                            return EvalResult::Error;
                        }
                    }
                }

                // Bind captured environment first — params shadow same-named captures
                for (name, cap_ref) in &captured {
                    self.scopes.declare(name.clone(), *cap_ref);
                }

                for (i, param) in parameters.iter().enumerate() {
                    let arg_data = self.resolve(arg_refs[i]).unwrap().clone();
                    let local_ref = self.alloc(arg_data);
                    self.scopes.declare(param.name.clone(), local_ref);
                }

                let mut result_ref = self.null_ref;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(_) => {} // implicit — function result is null unless explicit return
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v) => {
                            let owned = self.extract(v);
                            self.leave_call();
                            return EvalResult::Throw(self.plant(owned));
                        }
                        EvalResult::Error => {
                            self.leave_call();
                            return EvalResult::Error;
                        }
                        _ => { // Break/Continue inside a function body is an error
                            eprintln!("❌ ERROR: 'break'/'continue' cannot be used outside of a loop");
                            self.leave_call();
                            return EvalResult::Error;
                        }
                    }
                }

                // Deep-extract ANTES del pop — preserva elementos de arrays anidados
                let owned = self.extract(result_ref);

                self.leave_call(); // Flash Scope: destrucción instantánea de temporales
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
                let mut refs = Vec::new();
                for el in &arr.elements {
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
                            refs.push(r);
                        }
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array {
                    element_type: arr.element_type.clone(),
                    elements: refs,
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

                let left_data = self.resolve(left_ref).unwrap().clone();
                let idx_data = self.resolve(idx_ref).unwrap().clone();

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
                            eprintln!("❌ ERROR: Index out of bounds");
                            self.print_call_stack();
                            EvalResult::Error
                        } else {
                            EvalResult::Value(elements[*i as usize])
                        }
                    }
                    (ObjectData::Dict { entries, .. }, _) => {
                        let search_key = obj_data_to_key_str(&idx_data);
                        let found = entries.iter().find(|&&(k_ref, _)| {
                            let k_data = self.resolve(k_ref).unwrap();
                            obj_data_to_key_str(k_data) == search_key
                        });
                        match found {
                            Some(&(_, v_ref)) => EvalResult::Value(v_ref),
                            None => EvalResult::Value(self.null_ref),
                        }
                    }
                    _ => {
                        eprintln!("❌ ERROR: Index operator not supported for these types");
                        self.print_call_stack();
                        EvalResult::Error
                    }
                }
            }

            Expression::DictLiteral(dict_lit) => {
                let mut entries: Vec<(ObjectRef, ObjectRef)> = Vec::new();
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

                    entries.push((key_ref, val_ref));
                }
                EvalResult::Value(self.alloc(ObjectData::Dict {
                    key_type: dict_lit.key_type.clone(),
                    value_type: dict_lit.value_type.clone(),
                    entries,
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
                }

                // Detect chained mutation pattern: instance.field.mutate(args)
                // After mutation we write the modified array/dict back to the instance field
                let writeback_ctx: Option<(Expression, String)> =
                    if let Expression::DotCall(inner) = dot_call.object.as_ref() {
                        if inner.arguments.is_empty() {
                            const MUTATING: &[&str] = &["push", "pop", "shift", "unshift", "sort", "remove", "Add", "Remove", "RemoveAll", "clear"];
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

                let obj_data = match self.resolve(obj_ref) {
                    Some(d) => d.clone(),
                    None => {
                        eprintln!("❌ ERROR: Invalid reference in dot call");
                        return EvalResult::Error;
                    }
                };

                let result = match obj_data {
                    // ── Array methods ─────────────────────────────────────────
                    ObjectData::Array { element_type, elements: ref elems } => {
                        self.eval_array_method(obj_ref, element_type.clone(), elems.clone(), dot_call)
                    }

                    // ── String methods ────────────────────────────────────────
                    ObjectData::Str(ref s) => {
                        self.eval_string_method(s.clone(), dot_call)
                    }

                    // ── Dict methods ──────────────────────────────────────────
                    ObjectData::Dict { key_type, value_type, mut entries } => {
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
                                for (k_ref, v_ref) in entries.iter_mut() {
                                    let existing = self.resolve(*k_ref).unwrap().clone();
                                    if obj_data_to_key_str(&existing) == search_key {
                                        let owned_val = self.extract(val_ref);
                                        *v_ref = match obj_ref.region {
                                            RegionId::Global => self.plant_global(owned_val),
                                            RegionId::Scoped => self.plant(owned_val),
                                        };
                                        replaced = true;
                                        break;
                                    }
                                }
                                if !replaced {
                                    let owned_k = self.extract(key_ref);
                                    let owned_v = self.extract(val_ref);
                                    let new_k = match obj_ref.region {
                                        RegionId::Global => self.plant_global(owned_k),
                                        RegionId::Scoped => self.plant(owned_k),
                                    };
                                    let new_v = match obj_ref.region {
                                        RegionId::Global => self.plant_global(owned_v),
                                        RegionId::Scoped => self.plant(owned_v),
                                    };
                                    entries.push((new_k, new_v));
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

                                entries.retain(|(k_ref, _)| {
                                    let kd = self.resolve(*k_ref).unwrap();
                                    obj_data_to_key_str(kd) != search_key
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
                                    .map(|&(k, _)| self.extract(k))
                                    .collect();
                                let refs: Vec<ObjectRef> = keys.into_iter()
                                    .map(|v| self.plant(v))
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
                            }

                            "values" => {
                                let vals: Vec<OwnedValue> = entries.iter()
                                    .map(|&(_, v)| self.extract(v))
                                    .collect();
                                let refs: Vec<ObjectRef> = vals.into_iter()
                                    .map(|v| self.plant(v))
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
                            }

                            // Returns 2-D array of entries: [[k1,v1],[k2,v2],...]
                            "toArray" => {
                                let pairs: Vec<OwnedValue> = entries.iter()
                                    .map(|&(k, v)| OwnedValue::Array {
                                        element_type: None,
                                        elements: vec![self.extract(k), self.extract(v)],
                                    })
                                    .collect();
                                let rows: Vec<ObjectRef> = pairs.into_iter()
                                    .map(|row| self.plant(row))
                                    .collect();
                                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: rows }))
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
                    if let EvalResult::Value(inst_ref) = self.eval_expression(&inner_obj_expr) {
                        if let Some(ObjectData::Instance { class_name, mut fields }) = self.resolve(inst_ref).cloned() {
                            let updated = self.extract(obj_ref);
                            if let Some(f) = fields.iter_mut().find(|(n, _)| n == &field_name) {
                                f.1 = updated;
                            }
                            match inst_ref.region {
                                RegionId::Global => self.global_arena.update(inst_ref.index, ObjectData::Instance { class_name, fields }),
                                RegionId::Scoped => self.scopes.arena.update(inst_ref.index, ObjectData::Instance { class_name, fields }),
                            }
                        }
                    }
                }

                result
            }

            Expression::New(new_expr) => {
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
        }
    }

    fn eval_prefix(&mut self, op: &str, right_ref: ObjectRef, right: ObjectData) -> EvalResult {
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
                ObjectData::Boolean(b) => EvalResult::Value(self.alloc(ObjectData::Boolean(!b))),
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
            _ => EvalResult::Error,
        }
    }

    fn eval_infix(
        &mut self,
        op: &str,
        left: ObjectData,
        right: ObjectData,
        line: usize,
        column: usize,
    ) -> EvalResult {
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
                    EvalResult::Value(self.alloc(ObjectData::Boolean(eq)))
                }
                "!=" => {
                    let eq = matches!(left, ObjectData::Null) && matches!(right, ObjectData::Null);
                    EvalResult::Value(self.alloc(ObjectData::Boolean(!eq)))
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
                let result = match op {
                    "+" => match l.checked_add(r) {
                        Some(v) => ObjectData::Integer(v),
                        None => {
                            eprintln!("❌ ERROR: Integer overflow");
                            return EvalResult::Error;
                        }
                    },
                    "-" => match l.checked_sub(r) {
                        Some(v) => ObjectData::Integer(v),
                        None => {
                            eprintln!("❌ ERROR: Integer overflow");
                            return EvalResult::Error;
                        }
                    },
                    "*" => match l.checked_mul(r) {
                        Some(v) => ObjectData::Integer(v),
                        None => {
                            eprintln!("❌ ERROR: Integer overflow");
                            return EvalResult::Error;
                        }
                    },
                    "/" => {
                        if r == 0 {
                            eprintln!("❌ ERROR: Division by zero");
                            return EvalResult::Error;
                        }
                        match l.checked_div(r) {
                            Some(v) => ObjectData::Integer(v),
                            None => {
                                eprintln!("❌ ERROR: Integer overflow");
                                return EvalResult::Error;
                            }
                        }
                    }
                    "%" => {
                        if r == 0 {
                            eprintln!("❌ ERROR: Modulus operator by zero");
                            return EvalResult::Error;
                        }
                        match l.checked_rem(r) {
                            Some(v) => ObjectData::Integer(v),
                            None => {
                                eprintln!("❌ ERROR: Integer overflow");
                                return EvalResult::Error;
                            }
                        }
                    }
                    "<" => ObjectData::Boolean(l < r),
                    ">" => ObjectData::Boolean(l > r),
                    "<=" => ObjectData::Boolean(l <= r),
                    ">=" => ObjectData::Boolean(l >= r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => {
                        eprintln!("❌ ERROR: Unknown operator: {}", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            // Decimal arithmetic (decimal op decimal, int op decimal, decimal op int)
            (ObjectData::Decimal(l), ObjectData::Decimal(r)) => {
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
                    "<"  => ObjectData::Boolean(l < r),
                    ">"  => ObjectData::Boolean(l > r),
                    "<=" => ObjectData::Boolean(l <= r),
                    ">=" => ObjectData::Boolean(l >= r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => { eprintln!("❌ ERROR: Unknown operator: {}", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Integer(l), ObjectData::Decimal(r)) => {
                let l = l as f64;
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
                    "<"  => ObjectData::Boolean(l < r),
                    ">"  => ObjectData::Boolean(l > r),
                    "<=" => ObjectData::Boolean(l <= r),
                    ">=" => ObjectData::Boolean(l >= r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported here", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Decimal(l), ObjectData::Integer(r)) => {
                let r = r as f64;
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
                    "<"  => ObjectData::Boolean(l < r),
                    ">"  => ObjectData::Boolean(l > r),
                    "<=" => ObjectData::Boolean(l <= r),
                    ">=" => ObjectData::Boolean(l >= r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => { eprintln!("❌ ERROR: Operator '{}' not supported here", op); return EvalResult::Error; }
                };
                EvalResult::Value(self.alloc(result))
            }

            (ObjectData::Str(l), ObjectData::Str(r)) => {
                let result = match op {
                    "+"  => ObjectData::Str(l + &r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    "<"  => ObjectData::Boolean(l < r),
                    ">"  => ObjectData::Boolean(l > r),
                    "<=" => ObjectData::Boolean(l <= r),
                    ">=" => ObjectData::Boolean(l >= r),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between strings", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(s), ObjectData::Integer(n)) => {
                let result = match op {
                    "*" => {
                        if n < 0 {
                            eprintln!("❌ ERROR: Cannot repeat a string with a negative n");
                            return EvalResult::Error;
                        }
                        ObjectData::Str(s.repeat(n as usize))
                    }
                    "+" => ObjectData::Str(format!("{}{}", s, n)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between String and Integer", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Integer(n), ObjectData::Str(s)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}{}", n, s)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between Integer and String", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(s), ObjectData::Decimal(d)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}{}", s, format_decimal(d))),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between String and Decimal", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Decimal(d), ObjectData::Str(s)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}{}", format_decimal(d), s)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between Decimal and String", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(s), ObjectData::Boolean(b)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}{}", s, b)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between String and Boolean", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Boolean(b), ObjectData::Str(s)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}{}", b, s)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between Boolean and String", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(s), ObjectData::Null) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("{}null", s)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between String and Null", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Null, ObjectData::Str(s)) => {
                let result = match op {
                    "+" => ObjectData::Str(format!("null{}", s)),
                    "==" => ObjectData::Boolean(false),
                    "!=" => ObjectData::Boolean(true),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between Null and String", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Boolean(l), ObjectData::Boolean(r)) => {
                let result = match op {
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between booleans (use && / ||)", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
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
                if op == "==" { return EvalResult::Value(self.alloc(ObjectData::Boolean(false))); }
                if op == "!=" { return EvalResult::Value(self.alloc(ObjectData::Boolean(true))); }
                eprintln!(
                    "❌ ERROR: Type mismatch — operator '{}' cannot be applied between '{}' and '{}' - [{}:{}]",
                    op, left_type, right_type, line, column
                );
                self.print_call_stack();
                EvalResult::Error
            }
        }
    }

    fn is_truthy(&self, data: &ObjectData) -> bool {
        match data {
            ObjectData::Boolean(b) => *b,
            ObjectData::Null => false,
            _ => true,
        }
    }

    pub fn check_program(&self, program: &ast::Program) {
        println!("🚀 Starting static analysis (Flash Scope Criticality)...");
        println!(
            "⚠️  NOTE: Cost in bytes is an estimated value based on AST heuristics, not an exact runtime measurement.\n"
        );

        let mut total_memory = 0;

        for stmt in &program.statements {
            match stmt {
                ast::Statement::FunctionDeclaration(f) => {
                    self.analyze_function(&f.name, &f.function, &mut total_memory);
                }
                ast::Statement::Let(l) => {
                    if let ast::Expression::FunctionLiteral(func) = &l.value {
                        self.analyze_function(&l.name, func, &mut total_memory);
                    } else {
                        total_memory += estimate_expression(&l.value);
                    }
                }
                ast::Statement::Assign(a) => {
                    total_memory += estimate_expression(&a.value);
                }
                ast::Statement::Expression(e) => {
                    total_memory += estimate_expression(e);
                }
                _ => {}
            }
        }

        println!("📊 Estimated Global Memory: {} bytes", total_memory);
    }

    fn analyze_function(&self, name: &str, func: &ast::FunctionLiteral, total: &mut usize) {
        let mut local_mem = 0;

        // Estimar memoria de parámetros
        local_mem += func.parameters.len() * 8; // base

        // Estimar memoria del body
        for stmt in &func.body.statements {
            match stmt {
                ast::Statement::Let(l) => {
                    local_mem += 8; // variable pointer
                    local_mem += estimate_expression(&l.value);
                }
                ast::Statement::Assign(a) => {
                    local_mem += estimate_expression(&a.value);
                }
                ast::Statement::Expression(e) => {
                    local_mem += estimate_expression(e);
                }
                ast::Statement::Return(r) => {
                    local_mem += estimate_expression(&r.return_value);
                }
                ast::Statement::While(w) => {
                    local_mem += estimate_expression(&w.condition);
                    // For static analysis we approximate one iteration cost
                    for body_stmt in &w.body.statements {
                        if let ast::Statement::Expression(e) = body_stmt {
                            local_mem += estimate_expression(e);
                        } else if let ast::Statement::Let(l) = body_stmt {
                            local_mem += 8 + estimate_expression(&l.value);
                        }
                    }
                }
                ast::Statement::For(f) => {
                    local_mem += 8; // init variable
                    local_mem += estimate_expression(&f.condition);
                    local_mem += estimate_expression(&f.update.value);
                    // Approximate one iteration cost
                    for body_stmt in &f.body.statements {
                        if let ast::Statement::Expression(e) = body_stmt {
                            local_mem += estimate_expression(e);
                        } else if let ast::Statement::Let(l) = body_stmt {
                            local_mem += 8 + estimate_expression(&l.value);
                        }
                    }
                }
                ast::Statement::ForEach(fe) => {
                    local_mem += 8; // iteration variable
                    local_mem += estimate_expression(&fe.iterable);
                }
                _ => {}
            }
        }

        *total += local_mem;

        // Reporte de criticidad
        let (color, bar, level) = if local_mem < 1024 {
            ("\x1b[32m", "██", "🟢 < 1KB (Safe)")
        } else if local_mem < 10240 {
            ("\x1b[33m", "██████", "🟡 < 10KB (Warning)")
        } else {
            ("\x1b[31m", "██████████", "🔴 > 10KB (Critical)")
        };

        let reset = "\x1b[0m";
        println!("Function '{}': ~{} estimated bytes", name, local_mem);
        println!("  Criticality: {}{}{} {}\n", color, bar, reset, level);
    }

    fn update_dict(
        &mut self,
        obj_ref: ObjectRef,
        key_type: String,
        value_type: String,
        entries: Vec<(ObjectRef, ObjectRef)>,
    ) {
        let data = ObjectData::Dict { key_type, value_type, entries };
        match obj_ref.region {
            RegionId::Global => self.global_arena.update(obj_ref.index, data),
            RegionId::Scoped => self.scopes.arena.update(obj_ref.index, data),
        }
    }

    fn update_array(&mut self, arr_ref: ObjectRef, element_type: Option<String>, elems: Vec<ObjectRef>) {
        let data = ObjectData::Array { element_type, elements: elems };
        match arr_ref.region {
            RegionId::Global => self.global_arena.update(arr_ref.index, data),
            RegionId::Scoped => self.scopes.arena.update(arr_ref.index, data),
        }
    }

    fn leave_call(&mut self) {
        self.scopes.pop();
        self.call_depth -= 1;
        self.call_stack.pop();
    }

    fn print_call_stack(&self) {
        for frame in self.call_stack.iter().rev() {
            eprintln!("    called from '{}' [line {}:{}]", frame.name, frame.line, frame.column);
        }
        eprintln!();
    }

    fn plant_for_target(&mut self, value: OwnedValue, target_ref: ObjectRef) -> ObjectRef {
        match target_ref.region {
            RegionId::Global => self.plant_global(value),
            RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(value),
            RegionId::Scoped => self.plant(value),
        }
    }

    // ── fmt_value: display respecting op_str / op_str on array elements ────────

    /// Rich display: calls `op_str()` on Instances if defined, recurses into Arrays,
    /// falls back to `display()` for everything else.
    /// Returns `Err(EvalResult)` if `op_str` throws or errors, so callers can propagate.
    fn fmt_value(&mut self, obj_ref: ObjectRef) -> Result<String, EvalResult> {
        match self.resolve(obj_ref).cloned() {
            Some(ObjectData::Instance { ref class_name, .. }) => {
                let cn = class_name.clone();
                if self.find_method(&cn, "op_str").is_some() {
                    match self.call_op_method(obj_ref, &cn, "op_str", vec![], 0, 0) {
                        EvalResult::Value(r) => {
                            if let Some(ObjectData::Str(s)) = self.resolve(r) {
                                return Ok(s.clone());
                            }
                            Ok(self.display(r))
                        }
                        EvalResult::Throw(v) => Err(EvalResult::Throw(v)),
                        EvalResult::Error    => Err(EvalResult::Error),
                        other                => Err(other),
                    }
                } else {
                    Ok(self.display(obj_ref))
                }
            }
            Some(ObjectData::Array { elements, .. }) => {
                let mut parts = Vec::with_capacity(elements.len());
                for elem_ref in elements {
                    parts.push(self.fmt_value(elem_ref)?);
                }
                Ok(format!("[{}]", parts.join(", ")))
            }
            _ => Ok(self.display(obj_ref)),
        }
    }

    // ── Operator overloading dispatch ─────────────────────────────────────────

    /// Calls an `op_*` overload method on `inst_ref`. `arg_vals` are the already-extracted
    /// arguments (0 for unary like `op_neg`, 1 for binary like `op_add`).
    fn call_op_method(
        &mut self,
        inst_ref: ObjectRef,
        class_name: &str,
        method_name: &str,
        arg_vals: Vec<OwnedValue>,
        line: usize,
        column: usize,
    ) -> EvalResult {
        let method = match self.find_method(class_name, method_name) {
            Some(m) => m,
            None => {
                eprintln!("❌ ERROR: no operator overload '{}' on class '{}'", method_name, class_name);
                return EvalResult::Error;
            }
        };

        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }

        let old_executing_class = self.executing_class.take();
        self.executing_class = Some(class_name.to_string());
        self.call_stack.push(CallFrame {
            name: format!("{}::{}", class_name, method_name),
            line,
            column,
        });
        self.scopes.push();
        self.call_depth += 1;
        self.scopes.declare("this".to_string(), inst_ref);

        for (i, param) in method.parameters.iter().enumerate() {
            let arg_ref = if i < arg_vals.len() {
                self.plant(arg_vals[i].clone())
            } else {
                self.null_ref
            };
            self.scopes.declare(param.name.clone(), arg_ref);
        }

        let mut result_ref = self.null_ref;
        let mut error = false;
        let mut method_throw: Option<ObjectRef> = None;
        for stmt in &method.body.statements {
            match self.eval_statement(stmt) {
                EvalResult::Value(_)  => {}
                EvalResult::Return(v) => { result_ref = v; break; }
                EvalResult::Throw(v)  => { method_throw = Some(v); break; }
                EvalResult::Error     => { error = true; break; }
                EvalResult::Break | EvalResult::Continue => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop in operator method '{}'.", method_name);
                    error = true;
                    break;
                }
            }
        }

        let owned = self.extract(result_ref);
        let throw_owned = method_throw.map(|r| self.extract(r));
        self.leave_call();
        self.executing_class = old_executing_class;

        if error { return EvalResult::Error; }
        if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }
        EvalResult::Value(self.plant(owned))
    }

    // ── Callback calling helper ───────────────────────────────────────────────

    fn call_function(&mut self, func_ref: ObjectRef, arg_vals: Vec<OwnedValue>) -> EvalResult {
        if self.call_depth >= 1000 {
            eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
            return EvalResult::Error;
        }
        let func_data = self.resolve(func_ref).cloned();
        match func_data {
            Some(ObjectData::Function { parameters, body, captured, .. }) => {
                if arg_vals.len() != parameters.len() {
                    eprintln!(
                        "❌ ERROR: Callback expected {} argument(s), got {}",
                        parameters.len(), arg_vals.len()
                    );
                    return EvalResult::Error;
                }
                self.scopes.push();
                self.call_depth += 1;
                for (name, cap_ref) in &captured {
                    self.scopes.declare(name.clone(), *cap_ref);
                }
                for (param, val) in parameters.iter().zip(arg_vals.into_iter()) {
                    let r = self.plant(val);
                    self.scopes.declare(param.name.clone(), r);
                }
                let mut result_ref = self.null_ref;
                let mut fn_throw: Option<ObjectRef> = None;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(_) => {} // only explicit return contributes result
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v)  => { fn_throw = Some(v); break; }
                        EvalResult::Error => {
                            self.call_depth -= 1;
                            self.scopes.pop();
                            return EvalResult::Error;
                        }
                        EvalResult::Break | EvalResult::Continue => {
                            eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                            self.call_depth -= 1;
                            self.scopes.pop();
                            return EvalResult::Error;
                        }
                    }
                }
                if let Some(thrown) = fn_throw {
                    let owned = self.extract(thrown);
                    self.call_depth -= 1;
                    self.scopes.pop();
                    return EvalResult::Throw(self.plant(owned));
                }
                let owned = self.extract(result_ref);
                self.call_depth -= 1;
                self.scopes.pop();
                EvalResult::Value(self.plant(owned))
            }
            _ => {
                eprintln!("❌ ERROR: Callback is not a function");
                EvalResult::Error
            }
        }
    }

    fn callback_param_count(&self, func_ref: ObjectRef) -> Option<usize> {
        match self.resolve(func_ref) {
            Some(ObjectData::Function { parameters, .. }) => Some(parameters.len()),
            _ => None,
        }
    }

    // ── Built-in global functions ─────────────────────────────────────────────

    fn eval_assert(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.is_empty() || args.len() > 2 {
            eprintln!("❌ ERROR: assert(condition) or assert(condition, message)");
            return EvalResult::Error;
        }
        let cond_ref = match self.eval_expression(&args[0]) {
            EvalResult::Value(v) => v,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let is_true = matches!(self.resolve(cond_ref), Some(ObjectData::Boolean(true)));
        if !is_true {
            let msg = if args.len() == 2 {
                match self.eval_expression(&args[1]) {
                    EvalResult::Value(r) => self.display(r),
                    _ => "Assertion failed".to_string(),
                }
            } else {
                "Assertion failed".to_string()
            };
            let msg_ref = self.alloc(ObjectData::Str(msg));
            EvalResult::Throw(msg_ref)
        } else {
            EvalResult::Value(self.null_ref)
        }
    }

    fn eval_type_of(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: type_of expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(v) => v,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        let type_name = match self.resolve(r) {
            Some(ObjectData::Integer(_))  => "int",
            Some(ObjectData::Decimal(_))  => "decimal",
            Some(ObjectData::Boolean(_))  => "bool",
            Some(ObjectData::Str(_))      => "string",
            Some(ObjectData::Array { .. }) => "array",
            Some(ObjectData::Dict { .. }) => "dict",
            Some(ObjectData::Function { .. }) => "function",
            Some(ObjectData::Instance { class_name, .. }) => {
                // class_name vive en la arena, necesitamos clonar antes de alloc
                let name = class_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return EvalResult::Value(s);
            }
            Some(ObjectData::Null) | None => "null",
        };
        EvalResult::Value(self.alloc(ObjectData::Str(type_name.to_string())))
    }

    fn eval_parse_int(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: parseInt expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Integer(i))),
            Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Integer(d as i64))),
            Some(ObjectData::Str(s)) => match s.trim().parse::<i64>() {
                Ok(n) => EvalResult::Value(self.alloc(ObjectData::Integer(n))),
                Err(_) => {
                    eprintln!("❌ ERROR: parseInt: cannot parse '{}' as int", s);
                    EvalResult::Error
                }
            },
            _ => { eprintln!("❌ ERROR: parseInt: unsupported type"); EvalResult::Error }
        }
    }

    fn eval_parse_decimal(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: parseDecimal expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
            EvalResult::Throw(v) => return EvalResult::Throw(v),
            _ => return EvalResult::Error,
        };
        match self.resolve(r).cloned() {
            Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Decimal(i as f64))),
            Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Decimal(d))),
            Some(ObjectData::Str(s)) => match s.trim().parse::<f64>() {
                Ok(n) => EvalResult::Value(self.alloc(ObjectData::Decimal(n))),
                Err(_) => {
                    eprintln!("❌ ERROR: parseDecimal: cannot parse '{}' as decimal", s);
                    EvalResult::Error
                }
            },
            _ => { eprintln!("❌ ERROR: parseDecimal: unsupported type"); EvalResult::Error }
        }
    }

    // ── Math built-ins ────────────────────────────────────────────────────────

    fn eval_math_builtin(&mut self, name: &str, args: &[ast::Expression]) -> EvalResult {
        // Helper: resolve one numeric argument to f64
        let resolve_num = |evaluator: &mut Self, expr: &ast::Expression| -> Option<f64> {
            match evaluator.eval_expression(expr) {
                EvalResult::Value(r) => match evaluator.resolve(r).cloned() {
                    Some(ObjectData::Integer(i)) => Some(i as f64),
                    Some(ObjectData::Decimal(d)) => Some(d),
                    _ => { eprintln!("❌ ERROR: Math function '{}' expects numeric argument", name); None }
                },
                _ => None,
            }
        };

        match name {
            // --- Single-argument ---
            "abs" => {
                if args.len() != 1 { eprintln!("❌ ERROR: abs() expects 1 argument"); return EvalResult::Error; }
                let r = match self.eval_expression(&args[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                match self.resolve(r).cloned() {
                    Some(ObjectData::Integer(i)) => EvalResult::Value(self.alloc(ObjectData::Integer(i.abs()))),
                    Some(ObjectData::Decimal(d)) => EvalResult::Value(self.alloc(ObjectData::Decimal(d.abs()))),
                    _ => { eprintln!("❌ ERROR: abs() expects a numeric argument"); EvalResult::Error }
                }
            }
            "sqrt" => {
                if args.len() != 1 { eprintln!("❌ ERROR: sqrt() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v < 0.0 { eprintln!("❌ ERROR: sqrt() of negative number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.sqrt())))
            }
            "floor" => {
                if args.len() != 1 { eprintln!("❌ ERROR: floor() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.floor() as i64)))
            }
            "ceil" => {
                if args.len() != 1 { eprintln!("❌ ERROR: ceil() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.ceil() as i64)))
            }
            "round" => {
                if args.len() != 1 { eprintln!("❌ ERROR: round() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.round() as i64)))
            }
            "log" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.ln())))
            }
            "log2" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log2() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log2() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log2())))
            }
            "log10" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log10() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                if v <= 0.0 { eprintln!("❌ ERROR: log10() of non-positive number"); return EvalResult::Error; }
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log10())))
            }
            // --- Two-argument ---
            "min" => {
                if args.len() != 2 { eprintln!("❌ ERROR: min() expects 2 arguments"); return EvalResult::Error; }
                let ra = match self.eval_expression(&args[0]) { EvalResult::Value(r) => self.resolve(r).cloned(), _ => return EvalResult::Error };
                let rb = match self.eval_expression(&args[1]) { EvalResult::Value(r) => self.resolve(r).cloned(), _ => return EvalResult::Error };
                match (ra, rb) {
                    (Some(ObjectData::Integer(ia)), Some(ObjectData::Integer(ib))) =>
                        EvalResult::Value(self.alloc(ObjectData::Integer(ia.min(ib)))),
                    (Some(a), Some(b)) => {
                        let fa = match &a { ObjectData::Decimal(d) => *d, ObjectData::Integer(i) => *i as f64, _ => { eprintln!("❌ ERROR: min() expects numeric arguments"); return EvalResult::Error; } };
                        let fb = match &b { ObjectData::Decimal(d) => *d, ObjectData::Integer(i) => *i as f64, _ => { eprintln!("❌ ERROR: min() expects numeric arguments"); return EvalResult::Error; } };
                        EvalResult::Value(self.alloc(ObjectData::Decimal(fa.min(fb))))
                    }
                    _ => { eprintln!("❌ ERROR: min() expects numeric arguments"); EvalResult::Error }
                }
            }
            "max" => {
                if args.len() != 2 { eprintln!("❌ ERROR: max() expects 2 arguments"); return EvalResult::Error; }
                let ra = match self.eval_expression(&args[0]) { EvalResult::Value(r) => self.resolve(r).cloned(), _ => return EvalResult::Error };
                let rb = match self.eval_expression(&args[1]) { EvalResult::Value(r) => self.resolve(r).cloned(), _ => return EvalResult::Error };
                match (ra, rb) {
                    (Some(ObjectData::Integer(ia)), Some(ObjectData::Integer(ib))) =>
                        EvalResult::Value(self.alloc(ObjectData::Integer(ia.max(ib)))),
                    (Some(a), Some(b)) => {
                        let fa = match &a { ObjectData::Decimal(d) => *d, ObjectData::Integer(i) => *i as f64, _ => { eprintln!("❌ ERROR: max() expects numeric arguments"); return EvalResult::Error; } };
                        let fb = match &b { ObjectData::Decimal(d) => *d, ObjectData::Integer(i) => *i as f64, _ => { eprintln!("❌ ERROR: max() expects numeric arguments"); return EvalResult::Error; } };
                        EvalResult::Value(self.alloc(ObjectData::Decimal(fa.max(fb))))
                    }
                    _ => { eprintln!("❌ ERROR: max() expects numeric arguments"); EvalResult::Error }
                }
            }
            "pow" => {
                if args.len() != 2 { eprintln!("❌ ERROR: pow() expects 2 arguments (base, exp)"); return EvalResult::Error; }
                let base = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                let exp  = match resolve_num(self, &args[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(base.powf(exp))))
            }
            _ => { eprintln!("❌ ERROR: Unknown math function '{}'", name); EvalResult::Error }
        }
    }

    fn eval_read_line(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() > 1 {
            eprintln!("❌ ERROR: readLine expects 0 or 1 argument");
            return EvalResult::Error;
        }
        if let Some(prompt_expr) = args.first() {
            match self.eval_expression(prompt_expr) {
                EvalResult::Value(r) => {
                    let prompt = self.display(r);
                    print!("{}", prompt);
                    let _ = io::stdout().flush();
                }
                EvalResult::Throw(v) => return EvalResult::Throw(v),
                _ => return EvalResult::Error,
            }
        }
        let mut line = String::new();
        match io::stdin().read_line(&mut line) {
            Ok(_) => {
                let trimmed = line.trim_end_matches(['\n', '\r']).to_string();
                EvalResult::Value(self.alloc(ObjectData::Str(trimmed)))
            }
            Err(e) => {
                eprintln!("❌ ERROR: readLine: failed to read from stdin — {}", e);
                EvalResult::Error
            }
        }
    }

    // ── Interface / Class instantiation ──────────────────────────────────────

    fn eval_new_interface(&mut self, new_expr: &ast::NewExpression, iface_fields: Vec<ast::InterfaceField>) -> EvalResult {
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

    fn eval_new_class(&mut self, new_expr: &ast::NewExpression, class: StoredClass) -> EvalResult {
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

        // Allocate empty instance in current context
        let instance_ref = self.alloc(ObjectData::Instance {
            class_name: new_expr.class_name.clone(),
            fields: Vec::new(),
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
                    EvalResult::Break | EvalResult::Continue => {
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

    fn eval_super_call(&mut self, args: &[ast::Expression]) -> EvalResult {
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

        if arg_vals.len() != parent_ctor.parameters.len() {
            eprintln!("❌ ERROR: super() for '{}' expects {} arguments, got {}",
                parent_name, parent_ctor.parameters.len(), arg_vals.len());
            return EvalResult::Error;
        }

        // Execute parent constructor body — "this" is already bound in the current scope
        self.scopes.push();
        for (i, param) in parent_ctor.parameters.iter().enumerate() {
            let arg_ref = self.plant(arg_vals[i].clone());
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
                EvalResult::Break | EvalResult::Continue => {
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

    fn eval_super_method_call(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
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
                EvalResult::Break | EvalResult::Continue => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                    error = true;
                    break;
                }
            }
        }

        let owned = self.extract(result_ref);
        let throw_owned = method_throw.map(|r| self.extract(r));
        self.leave_call();
        self.executing_class = old_executing_class;

        if error { return EvalResult::Error; }
        if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }
        EvalResult::Value(self.plant(owned))
    }

    fn eval_object_patch(&mut self, var_name: &str, patch: Vec<(String, ast::Expression)>) -> EvalResult {
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

    fn eval_instance_dot(
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

                if arg_vals.len() != m.parameters.len() {
                    eprintln!("❌ ERROR: Method '{}' expects {} arguments, got {}",
                        method_name, m.parameters.len(), arg_vals.len());
                    return EvalResult::Error;
                }

                if !m.is_public && self.executing_class.as_deref() != Some(class_name.as_str()) {
                    eprintln!("❌ ERROR: Method '{}' is private and cannot be called externally", method_name);
                    return EvalResult::Error;
                }

                if self.call_depth >= 1000 {
                    eprintln!("❌ ERROR: Stack overflow — maximum call depth (1000) exceeded");
                    return EvalResult::Error;
                }

                let old_executing_class = self.executing_class.take();
                self.executing_class = Some(class_name.clone());

                self.call_stack.push(CallFrame {
                    name: format!("{}::{}", class_name, method_name),
                    line: dot_call.line,
                    column: dot_call.column,
                });
                self.scopes.push();
                self.call_depth += 1;
                self.scopes.declare("this".to_string(), obj_ref);

                for (i, param) in m.parameters.iter().enumerate() {
                    let arg_ref = self.plant(arg_vals[i].clone());
                    self.scopes.declare(param.name.clone(), arg_ref);
                }

                let mut result_ref = self.null_ref;
                let mut error = false;
                let mut method_throw: Option<ObjectRef> = None;
                for stmt in &m.body.statements {
                    match self.eval_statement(stmt) {
                        EvalResult::Value(_) => {} // implicit — method result is null unless explicit return
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v)  => { method_throw = Some(v); break; }
                        EvalResult::Error => { error = true; break; }
                        EvalResult::Break | EvalResult::Continue => {
                            eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop.");
                            error = true;
                            break;
                        }
                    }
                }

                let owned = self.extract(result_ref);
                let throw_owned = method_throw.map(|r| self.extract(r));
                self.leave_call();
                self.executing_class = old_executing_class;

                if error { return EvalResult::Error; }
                if let Some(t) = throw_owned { return EvalResult::Throw(self.plant(t)); }

                let result = self.plant(owned);

                // Validate return type if declared
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
    fn find_method(&self, class_name: &str, method_name: &str) -> Option<StoredMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.iter().find(|m| m.name == method_name) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    // ── Array methods ─────────────────────────────────────────────────────────

    fn eval_array_method(
        &mut self,
        arr_ref: ObjectRef,
        element_type: Option<String>,
        elems: Vec<ObjectRef>,
        dot_call: &ast::DotCallExpression,
    ) -> EvalResult {
        match dot_call.method.as_str() {

            "length" => EvalResult::Value(self.alloc(ObjectData::Integer(elems.len() as i64))),

            "push" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: push expects 1 argument");
                    return EvalResult::Error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if let Some(ref et) = element_type {
                    let data = self.resolve(val_ref).unwrap();
                    if !type_matches(et, data) {
                        eprintln!(
                            "❌ TYPE ERROR: Cannot push '{}' into [{}] array",
                            data.type_name(), et
                        );
                        return EvalResult::Error;
                    }
                }
                let val = self.extract(val_ref);
                let new_ref = match arr_ref.region {
                    RegionId::Global => self.plant_global(val),
                    RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(val),
                    RegionId::Scoped => self.plant(val),
                };
                let mut e = elems;
                e.push(new_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "pop" => {
                if elems.is_empty() {
                    return EvalResult::Value(self.null_ref);
                }
                let mut e = elems;
                let last = e.pop().unwrap();
                let owned = self.extract(last);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "shift" => {
                if elems.is_empty() {
                    return EvalResult::Value(self.null_ref);
                }
                let mut e = elems;
                let first = e.remove(0);
                let owned = self.extract(first);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "unshift" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: unshift expects 1 argument");
                    return EvalResult::Error;
                }
                let val_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                if let Some(ref et) = element_type {
                    let data = self.resolve(val_ref).unwrap();
                    if !type_matches(et, data) {
                        eprintln!(
                            "❌ TYPE ERROR: Cannot unshift '{}' into [{}] array",
                            data.type_name(), et
                        );
                        return EvalResult::Error;
                    }
                }
                let val = self.extract(val_ref);
                let new_ref = match arr_ref.region {
                    RegionId::Global => self.plant_global(val),
                    RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(val),
                    RegionId::Scoped => self.plant(val),
                };
                let mut e = elems;
                e.insert(0, new_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.null_ref)
            }

            "remove" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: remove expects 1 argument (index)");
                    return EvalResult::Error;
                }
                let idx = match self.eval_int_arg(&dot_call.arguments[0]) {
                    Some(v) => v,
                    None => return EvalResult::Error,
                };
                if elems.is_empty() {
                    return EvalResult::Value(self.null_ref);
                }
                if idx < 0 || idx as usize >= elems.len() {
                    eprintln!("❌ ERROR: remove: index {} out of bounds (length {})", idx, elems.len());
                    return EvalResult::Error;
                }
                let mut e = elems;
                let removed_ref = e.remove(idx as usize);
                let owned = self.extract(removed_ref);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "sort" => {
                // Evaluate the optional argument exactly once
                let arg_ref: Option<ObjectRef> = if dot_call.arguments.len() == 1 {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => Some(r),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                } else {
                    None
                };

                // If the argument is a function, use it as a comparator
                let is_comparator = arg_ref.is_some_and(|r| {
                    matches!(self.resolve(r), Some(ObjectData::Function { .. }))
                });

                if is_comparator {
                    let cb_ref = arg_ref.unwrap();
                    let mut owned_vals: Vec<OwnedValue> =
                        elems.iter().map(|&r| self.extract(r)).collect();
                    let n = owned_vals.len();
                    // Bubble sort (simple, avoids borrow issues with call_function)
                    let mut i = 0;
                    while i < n {
                        let mut j = 0;
                        while j < n - i - 1 {
                            let a = owned_vals[j].clone();
                            let b = owned_vals[j + 1].clone();
                            let result = self.call_function(cb_ref, vec![a, b]);
                            let should_swap = match result {
                                EvalResult::Value(r) => match self.resolve(r).cloned() {
                                    Some(ObjectData::Integer(v)) => v > 0,
                                    Some(ObjectData::Decimal(v)) => v > 0.0,
                                    _ => false,
                                },
                                _ => false,
                            };
                            if should_swap {
                                owned_vals.swap(j, j + 1);
                            }
                            j += 1;
                        }
                        i += 1;
                    }
                    let new_refs: Vec<ObjectRef> = owned_vals.into_iter().map(|v| {
                        match arr_ref.region {
                            RegionId::Global => self.plant_global(v),
                            RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(v),
                            RegionId::Scoped => self.plant(v),
                        }
                    }).collect();
                    self.update_array(arr_ref, element_type, new_refs);
                    return EvalResult::Value(arr_ref);
                }

                let order = match arg_ref {
                    None => "asc".to_string(),
                    Some(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(s)) => s,
                        _ => "asc".to_string(),
                    },
                };
                let descending = order == "desc";

                let mut owned_vals: Vec<OwnedValue> =
                    elems.iter().map(|&r| self.extract(r)).collect();

                let all_ints = owned_vals.iter().all(|v| matches!(v, OwnedValue::Integer(_)));
                let all_decs = owned_vals.iter().all(|v| matches!(v, OwnedValue::Decimal(_)));
                let all_strs = owned_vals.iter().all(|v| matches!(v, OwnedValue::Str(_)));

                if !all_ints && !all_decs && !all_strs {
                    eprintln!("❌ ERROR: sort requires a homogeneous array (all int, decimal, or string)");
                    return EvalResult::Error;
                }

                owned_vals.sort_by(|a, b| {
                    let cmp = match (a, b) {
                        (OwnedValue::Integer(x), OwnedValue::Integer(y)) => x.cmp(y),
                        (OwnedValue::Decimal(x), OwnedValue::Decimal(y)) =>
                            x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal),
                        (OwnedValue::Str(x), OwnedValue::Str(y)) => x.cmp(y),
                        _ => std::cmp::Ordering::Equal,
                    };
                    if descending { cmp.reverse() } else { cmp }
                });

                let new_refs: Vec<ObjectRef> = owned_vals.into_iter().map(|v| {
                    match arr_ref.region {
                        RegionId::Global => self.plant_global(v),
                        RegionId::Scoped if self.scopes.depth() > 1 => self.plant_global(v),
                        RegionId::Scoped => self.plant(v),
                    }
                }).collect();
                self.update_array(arr_ref, element_type, new_refs);
                EvalResult::Value(arr_ref)
            }

            "map" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: map expects 1 callback argument");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let n_params = match self.callback_param_count(cb_ref) {
                    Some(n) => n,
                    None => { eprintln!("❌ ERROR: map argument must be a function"); return EvalResult::Error; }
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut results = Vec::new();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let args = if n_params >= 2 {
                        vec![val, OwnedValue::Integer(i as i64)]
                    } else {
                        vec![val]
                    };
                    match self.call_function(cb_ref, args) {
                        EvalResult::Value(r) => results.push(r),
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: results }))
            }

            "filter" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: filter expects 1 callback argument");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let n_params = match self.callback_param_count(cb_ref) {
                    Some(n) => n,
                    None => { eprintln!("❌ ERROR: filter argument must be a function"); return EvalResult::Error; }
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut kept = Vec::new();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let args = if n_params >= 2 {
                        vec![val.clone(), OwnedValue::Integer(i as i64)]
                    } else {
                        vec![val.clone()]
                    };
                    let keep = match self.call_function(cb_ref, args) {
                        EvalResult::Value(r) => {
                            let d = self.resolve(r).cloned();
                            self.is_truthy(&d.unwrap_or(ObjectData::Null))
                        }
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if keep {
                        kept.push(self.plant(val));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array { element_type, elements: kept }))
            }

            "reduce" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: reduce expects 2 arguments (initial, callback)");
                    return EvalResult::Error;
                }
                let init_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let cb_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut acc_ref = init_ref;
                for val in owned_elems {
                    let acc_val = self.extract(acc_ref);
                    acc_ref = match self.call_function(cb_ref, vec![acc_val, val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                }
                EvalResult::Value(acc_ref)
            }

            "join" => {
                let sep = if dot_call.arguments.is_empty() {
                    ",".to_string()
                } else {
                    match self.eval_str_arg(&dot_call.arguments[0]) {
                        Some(s) => s,
                        None => return EvalResult::Error,
                    }
                };
                let parts: Vec<String> = elems.iter()
                    .map(|&r| self.display(r))
                    .collect();
                EvalResult::Value(self.alloc(ObjectData::Str(parts.join(&sep))))
            }

            "toString" => {
                let s = self.display(arr_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }

            "indexOf" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: indexOf expects 1 argument");
                    return EvalResult::Error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let idx = elems.iter().enumerate().find(|(_, elem)| {
                    let elem_data = self.resolve(**elem).cloned();
                    obj_data_eq(&elem_data, &needle_data)
                }).map(|(i, _)| i as i64).unwrap_or(-1);
                EvalResult::Value(self.alloc(ObjectData::Integer(idx)))
            }

            "includes" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: includes expects 1 argument");
                    return EvalResult::Error;
                }
                let needle_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let needle_data = self.resolve(needle_ref).cloned();
                let found = elems.iter().any(|elem| {
                    let elem_data = self.resolve(*elem).cloned();
                    obj_data_eq(&elem_data, &needle_data)
                });
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown array method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── String methods ────────────────────────────────────────────────────────

    fn eval_string_method(&mut self, s: String, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {

            "length" => EvalResult::Value(self.alloc(ObjectData::Integer(s.chars().count() as i64))),

            "toString" => EvalResult::Value(self.alloc(ObjectData::Str(s))),

            "trim" => EvalResult::Value(self.alloc(ObjectData::Str(s.trim().to_string()))),

            "toUpperCase" | "upper" => EvalResult::Value(self.alloc(ObjectData::Str(s.to_uppercase()))),

            "toLowerCase" | "lower" => EvalResult::Value(self.alloc(ObjectData::Str(s.to_lowercase()))),

            "startsWith" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: startsWith expects 1 argument");
                    return EvalResult::Error;
                }
                let prefix = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.starts_with(&prefix[..]))))
            }

            "endsWith" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: endsWith expects 1 argument");
                    return EvalResult::Error;
                }
                let suffix = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.ends_with(&suffix[..]))))
            }

            "indexOf" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: indexOf expects 1 argument (substring)");
                    return EvalResult::Error;
                }
                let needle = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let idx: i64 = if needle.is_empty() {
                    0
                } else {
                    // Search in character space, not byte space
                    let haystack: Vec<char> = s.chars().collect();
                    let needle_chars: Vec<char> = needle.chars().collect();
                    let mut found = -1i64;
                    'search: for i in 0..haystack.len() {
                        if haystack.len() - i < needle_chars.len() { break; }
                        for j in 0..needle_chars.len() {
                            if haystack[i + j] != needle_chars[j] { continue 'search; }
                        }
                        found = i as i64;
                        break;
                    }
                    found
                };
                EvalResult::Value(self.alloc(ObjectData::Integer(idx)))
            }

            "charAt" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: charAt expects 1 argument (index)");
                    return EvalResult::Error;
                }
                let idx = match self.eval_int_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let chars: Vec<char> = s.chars().collect();
                let result = if idx < 0 || idx as usize >= chars.len() {
                    String::new()
                } else {
                    chars[idx as usize].to_string()
                };
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "includes" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: includes expects 1 argument");
                    return EvalResult::Error;
                }
                let sub = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => match self.resolve(r).cloned() {
                        Some(ObjectData::Str(t)) => t,
                        _ => { eprintln!("❌ ERROR: includes argument must be a string"); return EvalResult::Error; }
                    },
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                EvalResult::Value(self.alloc(ObjectData::Boolean(s.contains(&sub[..]))))
            }

            "replace" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: replace expects 2 arguments (from, to)");
                    return EvalResult::Error;
                }
                let from = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let to   = match self.eval_str_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Str(s.replacen(&from[..], &to, 1))))
            }

            "replaceAll" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: replaceAll expects 2 arguments (from, to)");
                    return EvalResult::Error;
                }
                let from = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let to   = match self.eval_str_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Str(s.replace(&from[..], &to))))
            }

            "split" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: split expects 1 argument (separator)");
                    return EvalResult::Error;
                }
                let sep = match self.eval_str_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let parts: Vec<ObjectRef> = if sep.is_empty() {
                    // Empty separator → split into individual characters
                    s.chars().map(|c| self.alloc(ObjectData::Str(c.to_string()))).collect()
                } else {
                    s.split(&sep[..]).map(|p| self.alloc(ObjectData::Str(p.to_string()))).collect()
                };
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: parts }))
            }

            "substring" => {
                if dot_call.arguments.is_empty() || dot_call.arguments.len() > 2 {
                    eprintln!("❌ ERROR: substring expects 1 or 2 arguments (start [, end])");
                    return EvalResult::Error;
                }
                let start = match self.eval_int_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
                let end = if dot_call.arguments.len() == 2 {
                    match self.eval_int_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error }
                } else {
                    len
                };
                let start = start.max(0).min(len) as usize;
                let end   = end.max(0).min(len) as usize;
                let start = start.min(end);
                let result: String = chars[start..end].iter().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            _ => {
                eprintln!("❌ ERROR: Unknown string method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Argument extraction helpers ───────────────────────────────────────────

    fn eval_str_arg(&mut self, expr: &ast::Expression) -> Option<String> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Str(s)) => Some(s),
                _ => { eprintln!("❌ ERROR: Expected string argument"); None }
            },
            _ => None,
        }
    }

    fn eval_int_arg(&mut self, expr: &ast::Expression) -> Option<i64> {
        match self.eval_expression(expr) {
            EvalResult::Value(r) => match self.resolve(r).cloned() {
                Some(ObjectData::Integer(i)) => Some(i),
                _ => { eprintln!("❌ ERROR: Expected int argument"); None }
            },
            _ => None,
        }
    }

    // ── switch ────────────────────────────────────────────────────────────────

    fn eval_switch(&mut self, sw: &ast::SwitchStatement) -> EvalResult {
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

    fn values_equal(&self, a: &ObjectData, b: &ObjectData) -> bool {
        match (a, b) {
            (ObjectData::Integer(x),  ObjectData::Integer(y))  => x == y,
            (ObjectData::Decimal(x),  ObjectData::Decimal(y))  => x == y,
            // Cross-type numeric: same coercion that == uses in infix expressions
            (ObjectData::Decimal(x),  ObjectData::Integer(y))  => *x == (*y as f64),
            (ObjectData::Integer(x),  ObjectData::Decimal(y))  => (*x as f64) == *y,
            (ObjectData::Str(x),      ObjectData::Str(y))      => x == y,
            (ObjectData::Boolean(x),  ObjectData::Boolean(y))  => x == y,
            (ObjectData::Null,        ObjectData::Null)         => true,
            _ => false,
        }
    }

    // ── try / catch / finally ─────────────────────────────────────────────────

    fn eval_try(&mut self, try_stmt: &ast::TryStatement) -> EvalResult {
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
                    for s in &catch_block.statements {
                        match self.eval_statement(s) {
                            EvalResult::Value(v)   => catch_val = v,
                            EvalResult::Return(v)  => { catch_return   = Some(v); break; }
                            EvalResult::Throw(v)   => { catch_throw    = Some(v); break; }
                            EvalResult::Error      => { catch_error    = true;    break; }
                            EvalResult::Break      => { catch_break    = true;    break; }
                            EvalResult::Continue   => { catch_continue = true;    break; }
                        }
                    }
                    // Extract BEFORE pop so refs remain valid
                    let primary = catch_return.or(catch_throw).unwrap_or(catch_val);
                    let owned = self.extract(primary);
                    self.scopes.pop();
                    if catch_error             { EvalResult::Error }
                    else if catch_break        { EvalResult::Break }
                    else if catch_continue     { EvalResult::Continue }
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

// ── Free helpers ──────────────────────────────────────────────────────────────

/// Maps a binary operator symbol to its overload method name on instances.
/// Returns `""` for operators that cannot be overloaded.
fn operator_to_method_name(op: &str) -> &'static str {
    match op {
        "+"  => "op_add",
        "-"  => "op_sub",
        "*"  => "op_mul",
        "/"  => "op_div",
        "%"  => "op_mod",
        "==" => "op_eq",
        "!=" => "op_ne",
        "<"  => "op_lt",
        "<=" => "op_le",
        ">"  => "op_gt",
        ">=" => "op_ge",
        _    => "",
    }
}

/// Formats a decimal the same way `display()` does — trims trailing zeros beyond 10 decimal places.
fn format_decimal(d: f64) -> String {
    if d.fract() == 0.0 {
        format!("{:.1}", d)
    } else {
        let s = format!("{:.10}", d);
        s.trim_end_matches('0').trim_end_matches('.').to_string()
    }
}

fn obj_data_eq(a: &Option<ObjectData>, b: &Option<ObjectData>) -> bool {
    match (a, b) {
        (Some(ObjectData::Integer(x)),  Some(ObjectData::Integer(y)))  => x == y,
        (Some(ObjectData::Decimal(x)),  Some(ObjectData::Decimal(y)))  => x == y,
        (Some(ObjectData::Boolean(x)),  Some(ObjectData::Boolean(y)))  => x == y,
        (Some(ObjectData::Str(x)),      Some(ObjectData::Str(y)))      => x == y,
        (Some(ObjectData::Null),        Some(ObjectData::Null))        => true,
        _ => false,
    }
}

fn obj_data_to_key_str(data: &ObjectData) -> String {
    match data {
        ObjectData::Str(s) => s.clone(),
        ObjectData::Integer(i) => i.to_string(),
        ObjectData::Boolean(b) => b.to_string(),
        _ => String::new(),
    }
}

fn type_matches(expected: &str, data: &ObjectData) -> bool {
    match (expected, data) {
        ("int", ObjectData::Integer(_)) => true,
        ("decimal", ObjectData::Decimal(_)) => true,
        ("string", ObjectData::Str(_)) => true,
        ("bool", ObjectData::Boolean(_)) => true,
        ("void", ObjectData::Null) => true,
        ("dict", ObjectData::Dict { .. }) => true,
        ("array", ObjectData::Array { .. }) => true,
        ("any", _) => true,
        // "[type]" param accepts any array (element type enforced at construction)
        (t, ObjectData::Array { .. }) if t.starts_with('[') && t.ends_with(']') => true,
        // Nullable: "int?" accepts int or null
        (t, ObjectData::Null) if t.ends_with('?') => true,
        (t, d) if t.ends_with('?') => {
            let base = &t[..t.len() - 1];
            type_matches(base, d)
        }
        (t, ObjectData::Instance { class_name, .. }) => t == class_name.as_str(),
        _ => false,
    }
}

fn estimate_expression(expr: &ast::Expression) -> usize {
    match expr {
        ast::Expression::Integer(_) => 8,
        ast::Expression::Decimal(_) => 8,
        ast::Expression::Boolean(_) => 1,
        ast::Expression::String(s) => 24 + s.len(),
        ast::Expression::Identifier(_) => 8,
        ast::Expression::Lambda(_) => 32,
        ast::Expression::Prefix(_, right) => 8 + estimate_expression(right),
        ast::Expression::Infix(infix) => {
            8 + estimate_expression(&infix.left) + estimate_expression(&infix.right)
        }
        ast::Expression::FunctionLiteral(f) => 32 + f.parameters.len() * 8,
        ast::Expression::Call(c) => {
            let mut cost = 8;
            for arg in &c.arguments {
                cost += estimate_expression(arg);
            }
            cost
        }
        ast::Expression::ArrayLiteral(arr) => {
            let mut cost = 24;
            for item in &arr.elements {
                cost += estimate_expression(item);
            }
            cost
        }
        ast::Expression::Null => 0,
        ast::Expression::DictLiteral(d) => {
            let mut cost = 24;
            for (k, v) in &d.entries {
                cost += estimate_expression(k) + estimate_expression(v);
            }
            cost
        }
        ast::Expression::EntryLiteral(k, v) => estimate_expression(k) + estimate_expression(v),
        ast::Expression::DotCall(dc) => {
            let mut cost = 8;
            for arg in &dc.arguments {
                cost += estimate_expression(arg);
            }
            cost
        }
        ast::Expression::If(if_expr) => {
            let mut cost = estimate_expression(&if_expr.condition);
            let mut cons_cost = 0;
            for stmt in &if_expr.consequence.statements {
                if let ast::Statement::Expression(e) = stmt {
                    cons_cost += estimate_expression(e);
                } else if let ast::Statement::Let(l) = stmt {
                    cons_cost += 8 + estimate_expression(&l.value);
                }
            }
            let mut alt_cost = 0;
            if let Some(alt) = &if_expr.alternative {
                for stmt in &alt.statements {
                    if let ast::Statement::Expression(e) = stmt {
                        alt_cost += estimate_expression(e);
                    } else if let ast::Statement::Let(l) = stmt {
                        alt_cost += 8 + estimate_expression(&l.value);
                    }
                }
            }
            cost += std::cmp::max(cons_cost, alt_cost);
            cost
        }
        ast::Expression::Index(idx_expr) => {
            8 + estimate_expression(&idx_expr.left) + estimate_expression(&idx_expr.index)
        }
        ast::Expression::InterpolatedString(parts) => {
            let mut cost = 24usize;
            for part in parts {
                match part {
                    ast::StringPart::Literal(s) => cost += 24 + s.len(),
                    ast::StringPart::Expr(e) => cost += estimate_expression(e),
                }
            }
            cost
        }
        ast::Expression::New(n) => {
            let arg_cost: usize = match &n.args {
                ast::NewArgs::Positional(args) => args.iter().map(estimate_expression).sum(),
                ast::NewArgs::Fields(fields) => {
                    fields.iter().map(|(_, e)| estimate_expression(e)).sum()
                }
            };
            32 + arg_cost
        }
        ast::Expression::ObjectPatch(fields) => {
            32 + fields.iter().map(|(_, e)| estimate_expression(e)).sum::<usize>()
        }
        ast::Expression::Ternary(t) => {
            estimate_expression(&t.condition)
                + std::cmp::max(
                    estimate_expression(&t.then_expr),
                    estimate_expression(&t.else_expr),
                )
        }
    }
}
