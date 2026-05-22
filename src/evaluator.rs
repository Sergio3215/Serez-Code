use crate::ast::{self, Expression, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;

#[derive(Clone)]
struct StoredClass {
    parent: Option<String>,
    constructor: Option<ast::ClassConstructor>,
    methods: Vec<ast::ClassMethod>,
    is_abstract: bool,
    is_sealed: bool,
    fields: Vec<ast::ClassField>,
}

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),       // Ejecución normal (retorno implícito)
    Return(ObjectRef),      // Ejecución interrumpida por `return`
    Break,                  // Señal de break — capturada por while/for
    Continue,               // Señal de continue — capturada por while/for
    BreakLabel(String),     // Señal de break con label
    ContinueLabel(String),  // Señal de continue con label
    Error,                  // Ocurrió un error
    Throw(ObjectRef),       // Excepción de usuario — propagada hasta try/catch
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
    const_names: HashSet<String>,
    enum_registry: HashMap<String, Vec<String>>,
    sealed_classes: HashSet<String>,
    // LCG state for Math.random()
    lcg_state: u64,
    source_lines: Vec<String>,
}

impl Evaluator {
    pub fn new() -> Self {
        let mut global_arena = Arena::new();
        let null_idx = global_arena.alloc(ObjectData::Null);
        let null_ref = ObjectRef {
            region: RegionId::Global,
            index: null_idx,
        };

        // Seed LCG with current time
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(12345);

        Evaluator {
            global_arena,
            global_bindings: HashMap::new(),
            scopes: ScopeStack::new(),
            null_ref,
            call_stack: Vec::new(),
            interface_registry: HashMap::new(),
            class_registry: HashMap::new(),
            constructing_class: None,
            executing_class: None,
            call_depth: 0,
            const_names: HashSet::new(),
            enum_registry: HashMap::new(),
            sealed_classes: HashSet::new(),
            lcg_state: seed,
            source_lines: Vec::new(),
        }
    }

    pub fn set_source(&mut self, lines: Vec<String>) {
        self.source_lines = lines;
    }

    fn print_call_stack(&self) {
        for frame in self.call_stack.iter().rev() {
            eprintln!("    called from '{}' [line {}:{}]", frame.name, frame.line, frame.column);
            if let Some(src) = self.source_lines.get(frame.line.saturating_sub(1)) {
                let ln = frame.line.to_string();
                eprintln!("    {} | {}", ln, src.trim_end());
                eprintln!("    {}   {}^", " ".repeat(ln.len()), " ".repeat(frame.column.saturating_sub(1)));
            }
        }
        eprintln!();
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
    ///
    /// `rebind_outer`: when true (named `fn` declarations), the outer scope's
    /// binding is updated to point to the same global-arena slot so that mutations
    /// inside the function are visible to the caller (reference semantics).
    /// When false (lambda / arrow expressions), each capture is an independent copy
    /// — the outer variable is not aliased (value-snapshot semantics).
    ///
    /// Returns an empty vec at global scope (nothing to capture).
    fn capture_env(&mut self, rebind_outer: bool) -> Vec<(String, ObjectRef)> {
        let bindings = self.scopes.all_bindings();
        let mut result = Vec::new();
        for (name, r) in bindings {
            let global_ref = match r.region {
                RegionId::Global => r,
                RegionId::Scoped => {
                    let owned = self.extract(r);
                    let gref = self.plant_global(owned);
                    if rebind_outer {
                        self.scopes.rebind(&name, gref);
                    }
                    gref
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
            Some(ObjectData::Str(s)) => format!("{}", s),
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
            Some(ObjectData::EnumVariant { enum_name, variant }) => {
                format!("{}.{}", enum_name, variant)
            }
            Some(ObjectData::Set { elements: refs }) => {
                let elems: Vec<String> = refs.iter().map(|&r| self.display(r)).collect();
                format!("[{}]", elems.join(", "))
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
            Some(ObjectData::EnumVariant { enum_name, variant }) => OwnedValue::EnumVariant {
                enum_name: enum_name.clone(),
                variant: variant.clone(),
            },
            Some(ObjectData::Set { elements: refs }) => OwnedValue::Set {
                elements: refs.iter().map(|&r| self.extract(r)).collect(),
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
            OwnedValue::EnumVariant { enum_name, variant } => {
                self.alloc(ObjectData::EnumVariant { enum_name, variant })
            }
            OwnedValue::Set { elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant(v)).collect();
                self.alloc(ObjectData::Set { elements: refs })
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
            OwnedValue::EnumVariant { enum_name, variant } => {
                let idx = self.global_arena.alloc(ObjectData::EnumVariant { enum_name, variant });
                ObjectRef { region: RegionId::Global, index: idx }
            }
            OwnedValue::Set { elements: items } => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant_global(v)).collect();
                let idx = self.global_arena.alloc(ObjectData::Set { elements: refs });
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
                EvalResult::Break | EvalResult::BreakLabel(_) => {
                    if let Some(mark) = scratch_mark { self.global_arena.reset_to(mark); }
                    eprintln!("❌ FLASH SCOPE ERROR: 'break' cannot be used outside of a loop.");
                    return None;
                }
                EvalResult::Continue | EvalResult::ContinueLabel(_) => {
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

    fn eval_block(&mut self, block: &ast::BlockStatement) -> EvalResult {
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
                let captured = self.capture_env(false); // lambda: snapshot semantics
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
                let captured = self.capture_env(false); // arrow lambda: snapshot semantics
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
                        self.scopes.pop();
                        self.call_depth -= 1;
                        self.call_stack.pop();
                        return EvalResult::Error;
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
                                for elem_ref in spread_elems {
                                    arg_refs.push(elem_ref);
                                }
                            }
                            _ => {
                                eprintln!("❌ ERROR: Spread in function call requires an array");
                                self.scopes.pop(); self.call_depth -= 1; self.call_stack.pop();
                                return EvalResult::Error;
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
                let min_params = if has_rest { parameters.len().saturating_sub(1) } else { required_count };
                let max_params = if has_rest { usize::MAX } else { parameters.len() };

                if arg_refs.len() < min_params || arg_refs.len() > max_params {
                    let expected_str = if has_rest {
                        format!("at least {}", min_params)
                    } else if min_params == max_params {
                        format!("{}", min_params)
                    } else {
                        format!("{}-{}", min_params, max_params)
                    };
                    eprintln!(
                        "❌ ERROR: Function expected {} argument(s), got {}",
                        expected_str, arg_refs.len()
                    );
                    self.print_call_stack();
                    self.scopes.pop();
                    self.call_depth -= 1;
                    self.call_stack.pop();
                    return EvalResult::Error;
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
                for (name, cap_ref) in &captured {
                    self.scopes.declare(name.clone(), *cap_ref);
                }

                for (i, param) in parameters.iter().enumerate() {
                    if param.is_rest {
                        // Collect remaining args into an array
                        let rest_elems: Vec<ObjectRef> = arg_refs[i..].to_vec();
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

                let mut result_ref = self.null_ref;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(_) => {} // implicit — function result is null unless explicit return
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Throw(v) => {
                            let owned = self.extract(v);
                            self.scopes.pop();
                            self.call_depth -= 1;
                            self.call_stack.pop();
                            return EvalResult::Throw(self.plant(owned));
                        }
                        EvalResult::Error => {
                            self.scopes.pop();
                            self.call_depth -= 1;
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                        EvalResult::Break | EvalResult::Continue
                        | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                            eprintln!("❌ ERROR: 'break'/'continue' cannot be used outside of a loop");
                            self.scopes.pop();
                            self.call_depth -= 1;
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                    }
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
                let mut refs = Vec::new();
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
                                let owned_elems: Vec<OwnedValue> = spread_elems.iter()
                                    .map(|&r| self.extract(r))
                                    .collect();
                                for owned in owned_elems {
                                    refs.push(self.plant(owned));
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
                    // ── Namespace dispatch (Math / File / JSON) ───────────────
                    if name == "Math" {
                        return self.eval_math_namespace(dot_call);
                    }
                    if name == "File" {
                        return self.eval_file_namespace(dot_call);
                    }
                    if name == "JSON" {
                        return self.eval_json_namespace(dot_call);
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
                        if let Some(m) = class.methods.iter().find(|m| m.name == method_name && m.is_static).cloned() {
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

                    // ── Set methods ───────────────────────────────────────────
                    ObjectData::Set { elements } => {
                        self.eval_set_method(obj_ref, elements, dot_call)
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
                EvalResult::Value(self.alloc(ObjectData::Boolean(result)))
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
                    "**" => {
                        if r < 0 {
                            // negative exponent → fractional result → decimal
                            ObjectData::Decimal((l as f64).powi(r as i32))
                        } else {
                            ObjectData::Integer((l as f64).powi(r as i32) as i64)
                        }
                    }
                    "&"  => ObjectData::Integer(l & r),
                    "|"  => ObjectData::Integer(l | r),
                    "^"  => ObjectData::Integer(l ^ r),
                    "<<" => {
                        if r < 0 {
                            eprintln!("❌ ERROR: Left shift by negative amount ({})", r);
                            return EvalResult::Error;
                        }
                        if r >= 64 {
                            eprintln!("❌ ERROR: Left shift by {} is >= 64 bits", r);
                            return EvalResult::Error;
                        }
                        ObjectData::Integer(l << r)
                    }
                    ">>" => {
                        if r < 0 {
                            eprintln!("❌ ERROR: Right shift by negative amount ({})", r);
                            return EvalResult::Error;
                        }
                        if r >= 64 {
                            eprintln!("❌ ERROR: Right shift by {} is >= 64 bits", r);
                            return EvalResult::Error;
                        }
                        ObjectData::Integer(l >> r)
                    }
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
                    "**" => ObjectData::Decimal(l.powf(r)),
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
                    "**" => ObjectData::Decimal(l.powf(r)),
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
                    "**" => ObjectData::Decimal(l.powf(r)),
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
            // ── EnumVariant equality ─────────────────────────────────────────
            (ObjectData::EnumVariant { enum_name: en1, variant: v1 },
             ObjectData::EnumVariant { enum_name: en2, variant: v2 }) => {
                let eq = en1 == en2 && v1 == v2;
                let result = match op {
                    "==" => ObjectData::Boolean(eq),
                    "!=" => ObjectData::Boolean(!eq),
                    _ => {
                        eprintln!("❌ ERROR: Operator '{}' not supported between enum variants", op);
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
                        total_memory += self.estimate_expression(&l.value);
                    }
                }
                ast::Statement::Assign(a) => {
                    total_memory += self.estimate_expression(&a.value);
                }
                ast::Statement::Expression(e) => {
                    total_memory += self.estimate_expression(e);
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
                    local_mem += self.estimate_expression(&l.value);
                }
                ast::Statement::Assign(a) => {
                    local_mem += self.estimate_expression(&a.value);
                }
                ast::Statement::Expression(e) => {
                    local_mem += self.estimate_expression(e);
                }
                ast::Statement::Return(r) => {
                    local_mem += self.estimate_expression(&r.return_value);
                }
                ast::Statement::While(w) => {
                    local_mem += self.estimate_expression(&w.condition);
                    // For static analysis we approximate one iteration cost
                    for body_stmt in &w.body.statements {
                        if let ast::Statement::Expression(e) = body_stmt {
                            local_mem += self.estimate_expression(e);
                        } else if let ast::Statement::Let(l) = body_stmt {
                            local_mem += 8 + self.estimate_expression(&l.value);
                        }
                    }
                }
                ast::Statement::For(f) => {
                    local_mem += 8; // init variable
                    local_mem += self.estimate_expression(&f.condition);
                    local_mem += self.estimate_expression(&f.update.value);
                    // Approximate one iteration cost
                    for body_stmt in &f.body.statements {
                        if let ast::Statement::Expression(e) = body_stmt {
                            local_mem += self.estimate_expression(e);
                        } else if let ast::Statement::Let(l) = body_stmt {
                            local_mem += 8 + self.estimate_expression(&l.value);
                        }
                    }
                }
                ast::Statement::ForEach(fe) => {
                    local_mem += 8; // iteration variable
                    local_mem += self.estimate_expression(&fe.iterable);
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

    fn estimate_expression(&self, expr: &ast::Expression) -> usize {
        match expr {
            ast::Expression::Integer(_) => 8,
            ast::Expression::Decimal(_) => 8,
            ast::Expression::Boolean(_) => 1,
            ast::Expression::String(s) => 24 + s.len(),
            ast::Expression::Identifier(_) => 8,
            ast::Expression::Lambda(_) => 32,
            ast::Expression::Prefix(_, right) => 8 + self.estimate_expression(right),
            ast::Expression::Infix(infix) => {
                8 + self.estimate_expression(&infix.left) + self.estimate_expression(&infix.right)
            }
            ast::Expression::FunctionLiteral(f) => 32 + f.parameters.len() * 8,
            ast::Expression::Call(c) => {
                let mut cost = 8;
                for arg in &c.arguments {
                    cost += self.estimate_expression(arg);
                }
                cost
            }
            ast::Expression::ArrayLiteral(arr) => {
                let mut cost = 24;
                for item in &arr.elements {
                    cost += self.estimate_expression(item);
                }
                cost
            }
            ast::Expression::Null => 0,
            ast::Expression::DictLiteral(d) => {
                let mut cost = 24; // Vec overhead
                for (k, v) in &d.entries {
                    cost += self.estimate_expression(k) + self.estimate_expression(v);
                }
                cost
            }
            ast::Expression::EntryLiteral(k, v) => {
                self.estimate_expression(k) + self.estimate_expression(v)
            }
            ast::Expression::DotCall(dc) => {
                let mut cost = 8;
                for arg in &dc.arguments {
                    cost += self.estimate_expression(arg);
                }
                cost
            }
            ast::Expression::If(if_expr) => {
                let mut cost = self.estimate_expression(&if_expr.condition);
                let mut cons_cost = 0;
                for stmt in &if_expr.consequence.statements {
                    if let ast::Statement::Expression(e) = stmt {
                        cons_cost += self.estimate_expression(e);
                    } else if let ast::Statement::Let(l) = stmt {
                        cons_cost += 8 + self.estimate_expression(&l.value);
                    }
                }
                let mut alt_cost = 0;
                if let Some(alt) = &if_expr.alternative {
                    for stmt in &alt.statements {
                        if let ast::Statement::Expression(e) = stmt {
                            alt_cost += self.estimate_expression(e);
                        } else if let ast::Statement::Let(l) = stmt {
                            alt_cost += 8 + self.estimate_expression(&l.value);
                        }
                    }
                }
                cost += std::cmp::max(cons_cost, alt_cost);
                cost
            }
            ast::Expression::Index(idx_expr) => {
                8 + self.estimate_expression(&idx_expr.left)
                    + self.estimate_expression(&idx_expr.index)
            }
            ast::Expression::InterpolatedString(parts) => {
                let mut cost = 24usize;
                for part in parts {
                    match part {
                        ast::StringPart::Literal(s) => cost += 24 + s.len(),
                        ast::StringPart::Expr(e) => cost += self.estimate_expression(e),
                    }
                }
                cost
            }
            ast::Expression::New(n) => {
                let arg_cost: usize = match &n.args {
                    ast::NewArgs::Positional(args) => args.iter().map(|e| self.estimate_expression(e)).sum(),
                    ast::NewArgs::Fields(fields) => fields.iter().map(|(_, e)| self.estimate_expression(e)).sum(),
                };
                32 + arg_cost
            }
            ast::Expression::ObjectPatch(fields) => {
                32 + fields.iter().map(|(_, e)| self.estimate_expression(e)).sum::<usize>()
            }
            ast::Expression::Ternary(t) => {
                self.estimate_expression(&t.condition)
                    + std::cmp::max(
                        self.estimate_expression(&t.then_expr),
                        self.estimate_expression(&t.else_expr),
                    )
            }
            ast::Expression::Spread(inner) => self.estimate_expression(inner),
        }
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
                EvalResult::Break | EvalResult::Continue
                | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
                    eprintln!("❌ RUNTIME ERROR: break/continue used outside a loop in operator method '{}'.", method_name);
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
                        EvalResult::Break | EvalResult::Continue
                        | EvalResult::BreakLabel(_) | EvalResult::ContinueLabel(_) => {
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
            Some(ObjectData::EnumVariant { enum_name, .. }) => {
                let name = enum_name.clone();
                let s = self.alloc(ObjectData::Str(name));
                return EvalResult::Value(s);
            }
            Some(ObjectData::Set { .. }) => "Set",
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
                if args.is_empty() { eprintln!("❌ ERROR: min() expects at least 1 argument"); return EvalResult::Error; }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => { vals.push(i as f64); int_vals.push(i); }
                        Some(ObjectData::Decimal(d)) => { vals.push(d); all_int = false; }
                        _ => { eprintln!("❌ ERROR: min() expects numeric arguments"); return EvalResult::Error; }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    EvalResult::Value(self.alloc(ObjectData::Integer(*int_vals.iter().min().unwrap())))
                } else {
                    let m = vals.iter().cloned().fold(f64::INFINITY, f64::min);
                    EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
                }
            }
            "max" => {
                if args.is_empty() { eprintln!("❌ ERROR: max() expects at least 1 argument"); return EvalResult::Error; }
                let mut all_int = true;
                let mut vals: Vec<f64> = Vec::new();
                let mut int_vals: Vec<i64> = Vec::new();
                for arg in args {
                    let r = match self.eval_expression(arg) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(i)) => { vals.push(i as f64); int_vals.push(i); }
                        Some(ObjectData::Decimal(d)) => { vals.push(d); all_int = false; }
                        _ => { eprintln!("❌ ERROR: max() expects numeric arguments"); return EvalResult::Error; }
                    }
                }
                if all_int && int_vals.len() == args.len() {
                    EvalResult::Value(self.alloc(ObjectData::Integer(*int_vals.iter().max().unwrap())))
                } else {
                    let m = vals.iter().cloned().fold(f64::NEG_INFINITY, f64::max);
                    EvalResult::Value(self.alloc(ObjectData::Decimal(m)))
                }
            }
            "pow" => {
                if args.len() != 2 { eprintln!("❌ ERROR: pow() expects 2 arguments (base, exp)"); return EvalResult::Error; }
                let base = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                let exp  = match resolve_num(self, &args[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(base.powf(exp))))
            }
            "sin" => {
                if args.len() != 1 { eprintln!("❌ ERROR: sin() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.sin())))
            }
            "cos" => {
                if args.len() != 1 { eprintln!("❌ ERROR: cos() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.cos())))
            }
            "tan" => {
                if args.len() != 1 { eprintln!("❌ ERROR: tan() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.tan())))
            }
            "asin" => {
                if args.len() != 1 { eprintln!("❌ ERROR: asin() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.asin())))
            }
            "acos" => {
                if args.len() != 1 { eprintln!("❌ ERROR: acos() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.acos())))
            }
            "atan" => {
                if args.len() != 1 { eprintln!("❌ ERROR: atan() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.atan())))
            }
            "atan2" => {
                if args.len() != 2 { eprintln!("❌ ERROR: atan2() expects 2 arguments"); return EvalResult::Error; }
                let y = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                let x = match resolve_num(self, &args[1]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(y.atan2(x))))
            }
            "log2" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log2() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log2())))
            }
            "log10" => {
                if args.len() != 1 { eprintln!("❌ ERROR: log10() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.log10())))
            }
            "trunc" => {
                if args.len() != 1 { eprintln!("❌ ERROR: trunc() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Integer(v.trunc() as i64)))
            }
            "exp" => {
                if args.len() != 1 { eprintln!("❌ ERROR: exp() expects 1 argument"); return EvalResult::Error; }
                let v = match resolve_num(self, &args[0]) { Some(v) => v, None => return EvalResult::Error };
                EvalResult::Value(self.alloc(ObjectData::Decimal(v.exp())))
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
        for field in &class.fields.clone() {
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
    fn find_method(&self, class_name: &str, method_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.iter().find(|m| m.name == method_name && !m.is_getter && !m.is_setter) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    fn find_getter(&self, class_name: &str, prop_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.iter().find(|m| m.name == prop_name && m.is_getter) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    fn find_setter(&self, class_name: &str, prop_name: &str) -> Option<ast::ClassMethod> {
        let mut current = class_name.to_string();
        loop {
            let class = self.class_registry.get(&current)?;
            if let Some(m) = class.methods.iter().find(|m| m.name == prop_name && m.is_setter) {
                return Some(m.clone());
            }
            match &class.parent {
                Some(parent) => current = parent.clone(),
                None => return None,
            }
        }
    }

    // Shared helper: invoke a ClassMethod on an instance with pre-evaluated arg values.
    fn invoke_method(
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
                let is_comparator = arg_ref.map_or(false, |r| {
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

            "find" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: find expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let val_clone = val.clone();
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(&self.resolve(result).unwrap().clone()) {
                        return EvalResult::Value(self.plant(val_clone));
                    }
                }
                EvalResult::Value(self.null_ref)
            }

            "findIndex" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: findIndex expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for (i, val) in owned_elems.into_iter().enumerate() {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(&self.resolve(result).unwrap().clone()) {
                        return EvalResult::Value(self.alloc(ObjectData::Integer(i as i64)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Integer(-1)))
            }

            "slice" => {
                let len = elems.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => 0 },
                        _ => return EvalResult::Error,
                    }
                } else { 0 };
                let end_i = if dot_call.arguments.len() >= 2 {
                    match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => len },
                        _ => return EvalResult::Error,
                    }
                } else { len };
                // Normalize negative indices (count from end) then clamp
                let start = (if start_i < 0 { (len + start_i).max(0) } else { start_i.min(len) }) as usize;
                let end   = (if end_i   < 0 { (len + end_i  ).max(0) } else { end_i.min(len)   }) as usize;
                let end = end.max(start); // prevent inverted range
                let sliced: Vec<ObjectRef> = elems[start..end].iter().map(|r| {
                    let owned = self.extract(*r);
                    self.plant(owned)
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: element_type.clone(), elements: sliced }))
            }

            "reverse" => {
                let mut e = elems;
                e.reverse();
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(arr_ref)
            }

            "every" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: every expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if !self.is_truthy(&self.resolve(result).unwrap().clone()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(false)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(true)))
            }

            "some" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: some expects 1 argument (predicate)");
                    return EvalResult::Error;
                }
                let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(r) => r,
                    EvalResult::Throw(v) => return EvalResult::Throw(v),
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                for val in owned_elems {
                    let result = match self.call_function(cb_ref, vec![val]) {
                        EvalResult::Value(r) => r,
                        EvalResult::Throw(v) => return EvalResult::Throw(v),
                        _ => return EvalResult::Error,
                    };
                    if self.is_truthy(&self.resolve(result).unwrap().clone()) {
                        return EvalResult::Value(self.alloc(ObjectData::Boolean(true)));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Boolean(false)))
            }

            "flat" => {
                let depth = if dot_call.arguments.is_empty() {
                    1usize
                } else {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) {
                            Some(ObjectData::Integer(d)) => (*d).max(0) as usize,
                            _ => 1,
                        },
                        _ => return EvalResult::Error,
                    }
                };

                fn flat_owned(items: Vec<OwnedValue>, depth: usize) -> Vec<OwnedValue> {
                    if depth == 0 {
                        return items;
                    }
                    let mut result = Vec::new();
                    for item in items {
                        match item {
                            OwnedValue::Array { elements, .. } => {
                                result.extend(flat_owned(elements, depth - 1));
                            }
                            other => result.push(other),
                        }
                    }
                    result
                }

                let owned_elems: Vec<OwnedValue> = elems.iter().map(|r| self.extract(*r)).collect();
                let flat = flat_owned(owned_elems, depth);
                let refs: Vec<ObjectRef> = flat.into_iter().map(|v| self.plant(v)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
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

            "padStart" => {
                if dot_call.arguments.len() < 1 {
                    eprintln!("❌ ERROR: padStart expects at least 1 argument");
                    return EvalResult::Error;
                }
                let target_len = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i as usize, _ => 0 },
                    _ => return EvalResult::Error,
                };
                let pad_str = if dot_call.arguments.len() >= 2 {
                    self.eval_str_arg(&dot_call.arguments[1]).unwrap_or_else(|| " ".to_string())
                } else { " ".to_string() };
                let s_chars: Vec<char> = s.chars().collect();
                if s_chars.len() >= target_len {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                let mut result = s.clone();
                while result.chars().count() < target_len {
                    result = pad_str.clone() + &result;
                }
                // Trim to exact length if pad_str is multi-char and overshot
                let result: String = result.chars().rev().take(target_len).collect::<Vec<_>>().into_iter().rev().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "padEnd" => {
                if dot_call.arguments.len() < 1 {
                    eprintln!("❌ ERROR: padEnd expects at least 1 argument");
                    return EvalResult::Error;
                }
                let target_len = match self.eval_expression(&dot_call.arguments[0]) {
                    EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i as usize, _ => 0 },
                    _ => return EvalResult::Error,
                };
                let pad_str = if dot_call.arguments.len() >= 2 {
                    self.eval_str_arg(&dot_call.arguments[1]).unwrap_or_else(|| " ".to_string())
                } else { " ".to_string() };
                if s.chars().count() >= target_len {
                    return EvalResult::Value(self.alloc(ObjectData::Str(s.clone())));
                }
                let mut result = s.clone();
                while result.chars().count() < target_len {
                    result.push_str(&pad_str);
                }
                let result: String = result.chars().take(target_len).collect();
                EvalResult::Value(self.alloc(ObjectData::Str(result)))
            }

            "slice" => {
                let chars: Vec<char> = s.chars().collect();
                let slen = chars.len() as i64;
                let start_i = if !dot_call.arguments.is_empty() {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => 0 },
                        _ => return EvalResult::Error,
                    }
                } else { 0 };
                let end_i = if dot_call.arguments.len() >= 2 {
                    match self.eval_expression(&dot_call.arguments[1]) {
                        EvalResult::Value(v) => match self.resolve(v) { Some(ObjectData::Integer(i)) => *i, _ => slen },
                        _ => return EvalResult::Error,
                    }
                } else { slen };
                let start = (if start_i < 0 { (slen + start_i).max(0) } else { start_i.min(slen) }) as usize;
                let end   = (if end_i   < 0 { (slen + end_i  ).max(0) } else { end_i.min(slen)   }) as usize;
                let end = end.max(start);
                let sliced: String = chars[start..end].iter().collect();
                EvalResult::Value(self.alloc(ObjectData::Str(sliced)))
            }

            "trimStart" | "trimLeft" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.trim_start().to_string())))
            }

            "trimEnd" | "trimRight" => {
                EvalResult::Value(self.alloc(ObjectData::Str(s.trim_end().to_string())))
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
            (ObjectData::EnumVariant { enum_name: en1, variant: v1 },
             ObjectData::EnumVariant { enum_name: en2, variant: v2 }) => en1 == en2 && v1 == v2,
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

// ── Math namespace ─────────────────────────────────────────────────────────────

impl Evaluator {
    fn eval_math_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "PI" => EvalResult::Value(self.alloc(ObjectData::Decimal(std::f64::consts::PI))),
            "E"  => EvalResult::Value(self.alloc(ObjectData::Decimal(std::f64::consts::E))),
            "random" => {
                // LCG random: state = (a*state + c) mod m
                self.lcg_state = self.lcg_state.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                let val = (self.lcg_state >> 33) as f64 / (u32::MAX as f64);
                EvalResult::Value(self.alloc(ObjectData::Decimal(val)))
            }
            "clamp" => {
                if dot_call.arguments.len() != 3 {
                    eprintln!("❌ ERROR: Math.clamp(x, min, max) requires 3 arguments");
                    return EvalResult::Error;
                }
                let x   = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let mn  = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let mx  = match self.eval_expression(&dot_call.arguments[2]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                match (self.resolve(x).cloned(), self.resolve(mn).cloned(), self.resolve(mx).cloned()) {
                    (Some(ObjectData::Integer(xv)), Some(ObjectData::Integer(mnv)), Some(ObjectData::Integer(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Integer(xv.max(mnv).min(mxv)))),
                    (Some(ObjectData::Decimal(xv)), Some(ObjectData::Decimal(mnv)), Some(ObjectData::Decimal(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Decimal(xv.max(mnv).min(mxv)))),
                    (Some(ObjectData::Integer(xv)), Some(ObjectData::Integer(mnv)), Some(ObjectData::Decimal(mxv))) =>
                        EvalResult::Value(self.alloc(ObjectData::Decimal((xv as f64).max(mnv as f64).min(mxv)))),
                    _ => { eprintln!("❌ ERROR: Math.clamp requires numeric arguments"); EvalResult::Error }
                }
            }
            "sign" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Math.sign(x) requires 1 argument");
                    return EvalResult::Error;
                }
                let xr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                match self.resolve(xr).cloned() {
                    Some(ObjectData::Integer(v)) => {
                        let s = if v > 0 { 1i64 } else if v < 0 { -1 } else { 0 };
                        EvalResult::Value(self.alloc(ObjectData::Integer(s)))
                    }
                    Some(ObjectData::Decimal(v)) => {
                        let s = if v > 0.0 { 1i64 } else if v < 0.0 { -1 } else { 0 };
                        EvalResult::Value(self.alloc(ObjectData::Integer(s)))
                    }
                    _ => { eprintln!("❌ ERROR: Math.sign requires a numeric argument"); EvalResult::Error }
                }
            }
            // Delegate single-arg math functions to eval_math_builtin
            "abs" | "sqrt" | "floor" | "ceil" | "round" | "log" | "log2" | "log10"
            | "sin" | "cos" | "tan" | "asin" | "acos" | "atan" | "trunc" | "exp" => {
                self.eval_math_builtin(dot_call.method.as_str(), &dot_call.arguments)
            }
            // Multi-arg: min, max, pow, atan2
            "min" | "max" | "pow" | "atan2" => {
                self.eval_math_builtin(dot_call.method.as_str(), &dot_call.arguments)
            }
            _ => {
                eprintln!("❌ ERROR: Unknown Math method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── File namespace ─────────────────────────────────────────────────────────

    fn eval_file_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "exists" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.exists(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.exists requires a string path"); return EvalResult::Error; }
                };
                let exists = std::path::Path::new(&path).exists();
                EvalResult::Value(self.alloc(ObjectData::Boolean(exists)))
            }
            "read" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.read(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.read requires a string path"); return EvalResult::Error; }
                };
                match std::fs::read_to_string(&path) {
                    Ok(content) => EvalResult::Value(self.alloc(ObjectData::Str(content))),
                    Err(e) => { eprintln!("❌ ERROR: File error reading '{}': {}", path, e); EvalResult::Error }
                }
            }
            "write" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: File.write(path, content) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let cr = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.write path must be a string"); return EvalResult::Error; }
                };
                let content = self.display(cr);
                match std::fs::write(&path, &content) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: File error writing '{}': {}", path, e); EvalResult::Error }
                }
            }
            "create" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.create(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.create requires a string path"); return EvalResult::Error; }
                };
                // touch: create if not exists, leave untouched if exists
                if !std::path::Path::new(&path).exists() {
                    if let Err(e) = std::fs::File::create(&path) {
                        eprintln!("❌ ERROR: File error creating '{}': {}", path, e);
                        return EvalResult::Error;
                    }
                }
                EvalResult::Value(self.null_ref)
            }
            "read_asBinary" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: File.read_asBinary(path) requires 1 argument");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.read_asBinary requires a string path"); return EvalResult::Error; }
                };
                match std::fs::read(&path) {
                    Ok(bytes) => {
                        let refs: Vec<ObjectRef> = bytes.iter()
                            .map(|&b| self.alloc(ObjectData::Integer(b as i64)))
                            .collect();
                        EvalResult::Value(self.alloc(ObjectData::Array { element_type: Some("int".to_string()), elements: refs }))
                    }
                    Err(e) => { eprintln!("❌ ERROR: File error reading binary '{}': {}", path, e); EvalResult::Error }
                }
            }
            "write_asBinary" => {
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: File.write_asBinary(path, bytes) requires 2 arguments");
                    return EvalResult::Error;
                }
                let pr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let br = match self.eval_expression(&dot_call.arguments[1]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let path = match self.resolve(pr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: File.write_asBinary path must be a string"); return EvalResult::Error; }
                };
                let bytes_data = match self.resolve(br).cloned() {
                    Some(ObjectData::Array { elements, .. }) => elements,
                    _ => { eprintln!("❌ ERROR: File.write_asBinary bytes must be an array"); return EvalResult::Error; }
                };
                let mut buf: Vec<u8> = Vec::with_capacity(bytes_data.len());
                for r in bytes_data {
                    match self.resolve(r).cloned() {
                        Some(ObjectData::Integer(b)) if b >= 0 && b <= 255 => buf.push(b as u8),
                        _ => { eprintln!("❌ ERROR: File.write_asBinary: each byte must be int 0-255"); return EvalResult::Error; }
                    }
                }
                match std::fs::write(&path, &buf) {
                    Ok(_) => EvalResult::Value(self.null_ref),
                    Err(e) => { eprintln!("❌ ERROR: File error writing binary '{}': {}", path, e); EvalResult::Error }
                }
            }
            _ => {
                eprintln!("❌ ERROR: Unknown File method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── JSON namespace ─────────────────────────────────────────────────────────

    fn eval_json_namespace(&mut self, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "stringify" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: JSON.stringify(value) requires 1 argument");
                    return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let owned = self.extract(vr);
                let json = json_stringify_owned(&owned);
                EvalResult::Value(self.alloc(ObjectData::Str(json)))
            }
            "parse" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: JSON.parse(string) requires 1 argument");
                    return EvalResult::Error;
                }
                let sr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let s = match self.resolve(sr).cloned() {
                    Some(ObjectData::Str(s)) => s,
                    _ => { eprintln!("❌ ERROR: JSON.parse requires a string"); return EvalResult::Error; }
                };
                match json_parse(&s) {
                    Ok(owned) => EvalResult::Value(self.plant_global(owned)),
                    Err(e) => { eprintln!("❌ ERROR: JSON.parse error: {}", e); EvalResult::Error }
                }
            }
            _ => {
                eprintln!("❌ ERROR: Unknown JSON method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    // ── Set methods ────────────────────────────────────────────────────────────

    fn eval_new_set(&mut self, new_expr: &ast::NewExpression) -> EvalResult {
        let mut elements: Vec<ObjectRef> = Vec::new();
        let init_arg = match &new_expr.args {
            ast::NewArgs::Positional(pos_args) => pos_args.first().cloned(),
            ast::NewArgs::Fields(_) => None,
        };
        if let Some(init_expr) = init_arg {
            let arr_ref = match self.eval_expression(&init_expr) {
                EvalResult::Value(r) => r,
                _ => return EvalResult::Error,
            };
            if let Some(ObjectData::Array { elements: arr_elems, .. }) = self.resolve(arr_ref).cloned() {
                for elem_ref in arr_elems {
                    let elem_data = self.resolve(elem_ref).cloned();
                    let already = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &elem_data));
                    if !already {
                        elements.push(elem_ref);
                    }
                }
            }
        }
        EvalResult::Value(self.alloc(ObjectData::Set { elements }))
    }

    fn eval_set_method(&mut self, set_ref: ObjectRef, elements: Vec<ObjectRef>, dot_call: &ast::DotCallExpression) -> EvalResult {
        match dot_call.method.as_str() {
            "size" => EvalResult::Value(self.alloc(ObjectData::Integer(elements.len() as i64))),
            "toArray" => {
                let owned: Vec<OwnedValue> = elements.iter().map(|&r| self.extract(r)).collect();
                let refs: Vec<ObjectRef> = owned.into_iter().map(|v| self.plant(v)).collect();
                EvalResult::Value(self.alloc(ObjectData::Array { element_type: None, elements: refs }))
            }
            "has" | "contains" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.has(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let found = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &vd));
                EvalResult::Value(self.alloc(ObjectData::Boolean(found)))
            }
            "add" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.add(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let already = elements.iter().any(|&er| obj_data_eq(&self.resolve(er).cloned(), &vd));
                if !already {
                    let owned = self.extract(vr);
                    let new_ref = self.plant_global(owned);
                    let mut new_elems = elements;
                    new_elems.push(new_ref);
                    self.update_set(set_ref, new_elems);
                }
                EvalResult::Value(set_ref)
            }
            "delete" | "remove" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.delete(val) requires 1 argument"); return EvalResult::Error;
                }
                let vr = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let vd = self.resolve(vr).cloned();
                let before = elements.len();
                let new_elems: Vec<ObjectRef> = elements.into_iter().filter(|&er| !obj_data_eq(&self.resolve(er).cloned(), &vd)).collect();
                let removed = new_elems.len() < before;
                self.update_set(set_ref, new_elems);
                EvalResult::Value(self.alloc(ObjectData::Boolean(removed)))
            }
            "clear" => {
                self.update_set(set_ref, vec![]);
                EvalResult::Value(self.null_ref)
            }
            "union" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.union(other) requires 1 argument"); return EvalResult::Error;
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let other_elems = match self.resolve(or).cloned() {
                    Some(ObjectData::Set { elements: oe }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.union requires a Set argument"); return EvalResult::Error; }
                };
                let mut result: Vec<ObjectRef> = elements.clone();
                for er in other_elems {
                    let ed = self.resolve(er).cloned();
                    if !result.iter().any(|&rr| obj_data_eq(&self.resolve(rr).cloned(), &ed)) {
                        let owned = self.extract(er);
                        result.push(self.plant_global(owned));
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result }))
            }
            "intersection" => {
                if dot_call.arguments.len() != 1 {
                    eprintln!("❌ ERROR: Set.intersection(other) requires 1 argument"); return EvalResult::Error;
                }
                let or = match self.eval_expression(&dot_call.arguments[0]) { EvalResult::Value(r) => r, _ => return EvalResult::Error };
                let other_elems = match self.resolve(or).cloned() {
                    Some(ObjectData::Set { elements: oe }) => oe,
                    _ => { eprintln!("❌ ERROR: Set.intersection requires a Set argument"); return EvalResult::Error; }
                };
                let result: Vec<ObjectRef> = elements.into_iter().filter(|&er| {
                    let ed = self.resolve(er).cloned();
                    other_elems.iter().any(|&oer| obj_data_eq(&self.resolve(oer).cloned(), &ed))
                }).collect();
                EvalResult::Value(self.alloc(ObjectData::Set { elements: result }))
            }
            "toString" => {
                let s = self.display(set_ref);
                EvalResult::Value(self.alloc(ObjectData::Str(s)))
            }
            _ => {
                eprintln!("❌ ERROR: Unknown Set method '{}'", dot_call.method);
                EvalResult::Error
            }
        }
    }

    fn update_set(&mut self, set_ref: ObjectRef, new_elements: Vec<ObjectRef>) {
        let new_data = ObjectData::Set { elements: new_elements };
        match set_ref.region {
            RegionId::Global => self.global_arena.update(set_ref.index, new_data),
            RegionId::Scoped => self.scopes.arena.update(set_ref.index, new_data),
        }
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
        ("null", ObjectData::Null) => true,
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

// ── JSON helpers ────────────────────────────────────────────────────────────

fn json_stringify_owned(val: &OwnedValue) -> String {
    match val {
        OwnedValue::Null => "null".to_string(),
        OwnedValue::Boolean(b) => b.to_string(),
        OwnedValue::Integer(i) => i.to_string(),
        OwnedValue::Decimal(d) => {
            if d.fract() == 0.0 { format!("{:.1}", d) }
            else {
                let s = format!("{:.10}", d);
                s.trim_end_matches('0').trim_end_matches('.').to_string()
            }
        }
        OwnedValue::Str(s) => {
            let escaped = s
                .replace('\\', "\\\\")
                .replace('"', "\\\"")
                .replace('\n', "\\n")
                .replace('\r', "\\r")
                .replace('\t', "\\t");
            format!("\"{}\"", escaped)
        }
        OwnedValue::Array { elements, .. } => {
            let parts: Vec<String> = elements.iter().map(json_stringify_owned).collect();
            format!("[{}]", parts.join(","))
        }
        OwnedValue::Dict { entries, .. } => {
            let parts: Vec<String> = entries.iter().map(|(k, v)| {
                let key = match k {
                    OwnedValue::Str(s) => format!("\"{}\"", s.replace('"', "\\\"")),
                    OwnedValue::Integer(i) => format!("\"{}\"", i),
                    other => format!("\"{}\"", other.display_str()),
                };
                format!("{}:{}", key, json_stringify_owned(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
        OwnedValue::Instance { class_name: _, fields } => {
            let parts: Vec<String> = fields.iter().map(|(k, v)| {
                format!("\"{}\":{}", k.replace('"', "\\\""), json_stringify_owned(v))
            }).collect();
            format!("{{{}}}", parts.join(","))
        }
        OwnedValue::Set { elements } => {
            let parts: Vec<String> = elements.iter().map(json_stringify_owned).collect();
            format!("[{}]", parts.join(","))
        }
        OwnedValue::EnumVariant { enum_name, variant } => {
            format!("\"{}\"", format!("{}.{}", enum_name, variant).replace('"', "\\\""))
        }
        OwnedValue::Function { .. } => "null".to_string(),
    }
}

// A minimal recursive-descent JSON parser — no external crates.
fn json_parse(input: &str) -> Result<OwnedValue, String> {
    let chars: Vec<char> = input.chars().collect();
    let (val, pos) = json_parse_value(&chars, 0)?;
    let pos = json_skip_ws(&chars, pos);
    if pos != chars.len() {
        return Err(format!("unexpected trailing characters at position {}", pos));
    }
    Ok(val)
}

fn json_skip_ws(chars: &[char], mut pos: usize) -> usize {
    while pos < chars.len() && (chars[pos] == ' ' || chars[pos] == '\t' || chars[pos] == '\n' || chars[pos] == '\r') {
        pos += 1;
    }
    pos
}

fn json_parse_value(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let pos = json_skip_ws(chars, pos);
    if pos >= chars.len() {
        return Err("unexpected end of input".to_string());
    }
    match chars[pos] {
        '"'       => json_parse_string(chars, pos),
        '['       => json_parse_array(chars, pos),
        '{'       => json_parse_object(chars, pos),
        't'       => {
            if chars.get(pos..pos+4) == Some(&['t','r','u','e']) {
                Ok((OwnedValue::Boolean(true), pos + 4))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        'f'       => {
            if chars.get(pos..pos+5) == Some(&['f','a','l','s','e']) {
                Ok((OwnedValue::Boolean(false), pos + 5))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        'n'       => {
            if chars.get(pos..pos+4) == Some(&['n','u','l','l']) {
                Ok((OwnedValue::Null, pos + 4))
            } else { Err(format!("invalid token at {}", pos)) }
        }
        '-' | '0'..='9' => json_parse_number(chars, pos),
        c => Err(format!("unexpected character '{}' at position {}", c, pos)),
    }
}

fn json_parse_string(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    // pos points to opening '"'
    let mut i = pos + 1;
    let mut s = String::new();
    while i < chars.len() {
        match chars[i] {
            '"' => { return Ok((OwnedValue::Str(s), i + 1)); }
            '\\' => {
                i += 1;
                if i >= chars.len() { return Err("unterminated string escape".to_string()); }
                match chars[i] {
                    '"'  => s.push('"'),
                    '\\' => s.push('\\'),
                    '/'  => s.push('/'),
                    'n'  => s.push('\n'),
                    'r'  => s.push('\r'),
                    't'  => s.push('\t'),
                    'b'  => s.push('\u{0008}'),
                    'f'  => s.push('\u{000C}'),
                    'u'  => {
                        if i + 4 >= chars.len() { return Err("invalid \\u escape".to_string()); }
                        let hex: String = chars[i+1..i+5].iter().collect();
                        let code = u32::from_str_radix(&hex, 16)
                            .map_err(|_| format!("invalid \\u{}", hex))?;
                        let ch = char::from_u32(code)
                            .ok_or_else(|| format!("invalid unicode codepoint {}", code))?;
                        s.push(ch);
                        i += 4;
                    }
                    c => { s.push('\\'); s.push(c); }
                }
                i += 1;
            }
            c => { s.push(c); i += 1; }
        }
    }
    Err("unterminated string".to_string())
}

fn json_parse_array(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = json_skip_ws(chars, pos + 1); // skip '['
    let mut elements = Vec::new();
    if i < chars.len() && chars[i] == ']' {
        return Ok((OwnedValue::Array { element_type: None, elements }, i + 1));
    }
    loop {
        let (val, next) = json_parse_value(chars, i)?;
        elements.push(val);
        i = json_skip_ws(chars, next);
        if i >= chars.len() { return Err("unterminated array".to_string()); }
        match chars[i] {
            ']' => { return Ok((OwnedValue::Array { element_type: None, elements }, i + 1)); }
            ',' => { i = json_skip_ws(chars, i + 1); }
            c   => { return Err(format!("expected ',' or ']', got '{}'", c)); }
        }
    }
}

fn json_parse_object(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = json_skip_ws(chars, pos + 1); // skip '{'
    let mut entries: Vec<(OwnedValue, OwnedValue)> = Vec::new();
    if i < chars.len() && chars[i] == '}' {
        return Ok((OwnedValue::Dict { key_type: "string".to_string(), value_type: "any".to_string(), entries }, i + 1));
    }
    loop {
        i = json_skip_ws(chars, i);
        if i >= chars.len() || chars[i] != '"' {
            return Err(format!("expected string key at position {}", i));
        }
        let (key, next_k) = json_parse_string(chars, i)?;
        i = json_skip_ws(chars, next_k);
        if i >= chars.len() || chars[i] != ':' {
            return Err(format!("expected ':' at position {}", i));
        }
        i = json_skip_ws(chars, i + 1);
        let (val, next_v) = json_parse_value(chars, i)?;
        entries.push((key, val));
        i = json_skip_ws(chars, next_v);
        if i >= chars.len() { return Err("unterminated object".to_string()); }
        match chars[i] {
            '}' => { return Ok((OwnedValue::Dict { key_type: "string".to_string(), value_type: "any".to_string(), entries }, i + 1)); }
            ',' => { i = json_skip_ws(chars, i + 1); }
            c   => { return Err(format!("expected ',' or '}}', got '{}'", c)); }
        }
    }
}

fn json_parse_number(chars: &[char], pos: usize) -> Result<(OwnedValue, usize), String> {
    let mut i = pos;
    let mut s = String::new();
    if i < chars.len() && chars[i] == '-' { s.push('-'); i += 1; }
    while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    let is_float = i < chars.len() && (chars[i] == '.' || chars[i] == 'e' || chars[i] == 'E');
    if i < chars.len() && chars[i] == '.' {
        s.push('.');
        i += 1;
        while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    }
    if i < chars.len() && (chars[i] == 'e' || chars[i] == 'E') {
        s.push(chars[i]); i += 1;
        if i < chars.len() && (chars[i] == '+' || chars[i] == '-') { s.push(chars[i]); i += 1; }
        while i < chars.len() && chars[i].is_ascii_digit() { s.push(chars[i]); i += 1; }
    }
    if is_float || s.contains('.') || s.contains('e') || s.contains('E') {
        let f: f64 = s.parse().map_err(|_| format!("invalid number '{}'", s))?;
        Ok((OwnedValue::Decimal(f), i))
    } else {
        let n: i64 = s.parse().map_err(|_| format!("invalid integer '{}'", s))?;
        Ok((OwnedValue::Integer(n), i))
    }
}
