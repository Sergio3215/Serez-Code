use crate::ast::{self, Expression, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::HashMap;

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),  // Ejecución normal (retorno implícito)
    Return(ObjectRef), // Ejecución interrumpida por `return`
    Error,             // Ocurrió un error
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
}

impl Evaluator {
    pub fn new() -> Self {
        let mut global_arena = Arena::new();
        let null_idx = global_arena.alloc(ObjectData::Null);
        let null_ref = ObjectRef {
            region: RegionId::Global,
            index: null_idx,
        };

        Evaluator {
            global_arena,
            global_bindings: HashMap::new(),
            scopes: ScopeStack::new(),
            null_ref,
            call_stack: Vec::new(),
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

    /// Snapshot all currently-visible scoped variables as arena-independent
    /// OwnedValues for use as a lexical closure capture.
    /// Returns an empty vec at global scope (nothing to capture).
    fn capture_env(&self) -> Vec<(String, OwnedValue)> {
        self.scopes
            .all_bindings()
            .into_iter()
            .map(|(name, r)| (name, self.extract(r)))
            .collect()
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
                captured: captured.clone(),
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
                    if let Some(mark) = scratch_mark {
                        self.global_arena.reset_to(mark);
                    }
                    eprintln!(
                        "❌ FLASH SCOPE ERROR: 'return' cannot be used outside of a function or conditional or loops."
                    );
                    return None;
                }
                EvalResult::Error => {
                    if let Some(mark) = scratch_mark {
                        self.global_arena.reset_to(mark);
                    }
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

                let val_ref = match self.eval_expression(&assign_stmt.value) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let new_data = self.resolve(val_ref).unwrap().clone();

                if self.scopes.assign(&assign_stmt.name, new_data.clone()) {
                    return EvalResult::Value(self.scopes.lookup(&assign_stmt.name).unwrap());
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
                    body: func_decl.function.body.clone(),
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
                        EvalResult::Return(v) => {
                            // Un return dentro de un while interrumpe el while y sube el return
                            return EvalResult::Return(v);
                        }
                        EvalResult::Error => return EvalResult::Error,
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
                    EvalResult::Error => {
                        self.scopes.pop();
                        return EvalResult::Error;
                    }
                    EvalResult::Return(v) => {
                        self.scopes.pop();
                        return EvalResult::Return(v);
                    }
                };
                self.scopes.declare(for_stmt.init.name.clone(), init_val);

                // loop_return holds an extracted (arena-independent) return value if a
                // `return` was encountered inside the body — extracted BEFORE the for-scope
                // is popped so the ObjectRef is still valid at extraction time.
                let mut loop_return: Option<OwnedValue> = None;
                let mut loop_error = false;

                loop {
                    // Evaluate condition, free its temporary immediately
                    let cond_mark = self.scopes.arena.watermark();
                    let condition_ref = match self.eval_expression(&for_stmt.condition) {
                        EvalResult::Value(v) => v,
                        EvalResult::Error => {
                            loop_error = true;
                            break;
                        }
                        EvalResult::Return(v) => {
                            loop_return = Some(self.extract(v));
                            break;
                        }
                    };
                    let condition_data = self.resolve(condition_ref).unwrap().clone();
                    self.scopes.arena.reset_to(cond_mark);

                    if !self.is_truthy(&condition_data) {
                        break;
                    }

                    // Execute body — eval_block handles its own push/pop
                    match self.eval_block(&for_stmt.body) {
                        EvalResult::Value(_) => {}
                        EvalResult::Return(v) => {
                            // Extract while for-scope (and its sub-allocs) is still live
                            loop_return = Some(self.extract(v));
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

                    if self.scopes.assign(&for_stmt.update.name, new_data.clone()) {
                        // updated in-place in scoped arena
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

                // Pop the for-scope AFTER extracting any return value above
                self.scopes.pop();

                if loop_error {
                    return EvalResult::Error;
                }
                if let Some(owned) = loop_return {
                    return EvalResult::Return(self.plant(owned));
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::IndexAssign(stmt) => {
                // Resolve the target array
                let arr_ref = match self.lookup_var(&stmt.target) {
                    Some(r) => r,
                    None => {
                        eprintln!("❌ ERROR: Variable not found: {}", stmt.target);
                        return EvalResult::Error;
                    }
                };

                // Evaluate index and new value
                let idx_ref = match self.eval_expression(&stmt.index) {
                    EvalResult::Value(v) => v,
                    _ => return EvalResult::Error,
                };
                let val_ref = match self.eval_expression(&stmt.value) {
                    EvalResult::Value(v) => v,
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
                        let search_key = obj_data_to_key_str(&idx_data);
                        let owned_val = self.extract(val_ref);

                        // Index-based loop so we can call &mut self methods (plant/plant_global)
                        let mut replaced = false;
                        let mut i = 0;
                        while i < entries.len() {
                            let k_data = self.resolve(entries[i].0).unwrap().clone();
                            if obj_data_to_key_str(&k_data) == search_key {
                                let new_ref = match arr_ref.region {
                                    RegionId::Global => self.plant_global(owned_val.clone()),
                                    RegionId::Scoped => self.plant(owned_val.clone()),
                                };
                                entries[i].1 = new_ref;
                                replaced = true;
                                break;
                            }
                            i += 1;
                        }
                        if !replaced {
                            let owned_k = OwnedValue::Str(search_key);
                            let new_k = match arr_ref.region {
                                RegionId::Global => self.plant_global(owned_k),
                                RegionId::Scoped => self.plant(owned_k),
                            };
                            let new_v = match arr_ref.region {
                                RegionId::Global => self.plant_global(owned_val),
                                RegionId::Scoped => self.plant(owned_val),
                            };
                            entries.push((new_k, new_v));
                        }
                        self.update_dict(arr_ref, key_type, value_type, entries);
                    }

                    _ => {
                        eprintln!("❌ ERROR: '{}' is not an array or dict", stmt.target);
                        return EvalResult::Error;
                    }
                }

                EvalResult::Value(self.null_ref)
            }

            Statement::Return(return_stmt) => {
                match self.eval_expression(&return_stmt.return_value) {
                    EvalResult::Value(v) => EvalResult::Return(v),
                    _ => EvalResult::Error,
                }
            }

            Statement::Out(out_stmt) => match self.eval_expression(&out_stmt.value) {
                EvalResult::Value(v) => {
                    println!("{}", self.display(v));
                    EvalResult::Value(self.null_ref)
                }
                EvalResult::Return(v) => EvalResult::Return(v),
                EvalResult::Error => EvalResult::Error,
            },

            Statement::Expression(expr) => self.eval_expression(expr),
        }
    }

    fn eval_block(&mut self, block: &ast::BlockStatement) -> EvalResult {
        self.scopes.push();
        let mut result = EvalResult::Value(self.null_ref);

        for s in &block.statements {
            match self.eval_statement(s) {
                EvalResult::Value(v) => result = EvalResult::Value(v),
                EvalResult::Return(v) => {
                    result = EvalResult::Return(v);
                    break;
                }
                EvalResult::Error => {
                    result = EvalResult::Error;
                    break;
                }
            }
        }

        // Deep-extract ANTES del pop: preserva elementos de arrays y valores anidados.
        let owned = match &result {
            EvalResult::Value(v) | EvalResult::Return(v) => Some(self.extract(*v)),
            EvalResult::Error => None,
        };

        self.scopes.pop();

        match owned {
            Some(val) => {
                let promoted = self.plant(val);
                match result {
                    EvalResult::Value(_) => EvalResult::Value(promoted),
                    EvalResult::Return(_) => EvalResult::Return(promoted),
                    EvalResult::Error => unreachable!(),
                }
            }
            None => EvalResult::Error,
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
                    body: func_lit.body.clone(),
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
                    body,
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
                                EvalResult::Value(r) => result.push_str(&self.display(r)),
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
                        "parseInt" => return self.eval_parse_int(&call_expr.arguments),
                        "parseDecimal" => return self.eval_parse_decimal(&call_expr.arguments),
                        _ => {}
                    }
                }

                let func_ref = match self.eval_expression(&call_expr.function) {
                    EvalResult::Value(r) => r,
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
                        self.call_stack.pop();
                        return EvalResult::Error;
                    }
                };

                let mut arg_refs = Vec::new();
                for arg in &call_expr.arguments {
                    match self.eval_expression(arg) {
                        EvalResult::Value(r) => arg_refs.push(r),
                        _ => {
                            self.scopes.pop();
                            self.call_stack.pop();
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

                    for frame in self.call_stack.iter().rev() {
                        eprintln!(
                            "    called from '{}' [line {}:{}]",
                            frame.name, frame.line, frame.column
                        );
                    }
                    eprintln!();
                    self.scopes.pop();
                    self.call_stack.pop();
                    return EvalResult::Error;
                }

                for (i, param) in parameters.iter().enumerate() {
                    let arg_ref = arg_refs[i];
                    if let Some(expected_type) = &param.type_name {
                        let actual_data = self.resolve(arg_ref).unwrap();
                        let is_valid = type_matches(expected_type.as_str(), actual_data);

                        if !is_valid {
                            eprintln!(
                                "❌ TYPE ERROR: Parameter '{}' expected '{}' but received another type.",
                                param.name, expected_type
                            );

                            for frame in self.call_stack.iter().rev() {
                                eprintln!(
                                    "    called from '{}' [line {}:{}]",
                                    frame.name, frame.line, frame.column
                                );
                            }
                            eprintln!();
                            self.scopes.pop();
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                    }
                }

                // Bind captured environment first — params shadow same-named captures
                for (name, owned) in captured {
                    let local_ref = self.plant(owned);
                    self.scopes.declare(name, local_ref);
                }

                for (i, param) in parameters.iter().enumerate() {
                    let arg_data = self.resolve(arg_refs[i]).unwrap().clone();
                    let local_ref = self.alloc(arg_data);
                    self.scopes.declare(param.name.clone(), local_ref);
                }

                let mut result_ref = self.null_ref;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(v) => result_ref = v,
                        EvalResult::Return(v) => {
                            result_ref = v;
                            break;
                        }
                        EvalResult::Error => {
                            self.scopes.pop();
                            self.call_stack.pop();
                            return EvalResult::Error;
                        }
                    }
                } // <--- Added missing brace

                // Deep-extract ANTES del pop — preserva elementos de arrays anidados
                let owned = self.extract(result_ref);

                self.scopes.pop(); // Flash Scope: destrucción instantánea de temporales
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
                        for frame in self.call_stack.iter().rev() {
                            eprintln!(
                                "    called from '{}' [line {}:{}]",
                                frame.name, frame.line, frame.column
                            );
                        }
                        eprintln!();
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
                    EvalResult::Error => return EvalResult::Error,
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
                    _ => return EvalResult::Error,
                };
                let idx_ref = match self.eval_expression(&index_expr.index) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };

                let left_data = self.resolve(left_ref).unwrap().clone();
                let idx_data = self.resolve(idx_ref).unwrap().clone();

                match (&left_data, &idx_data) {
                    (ObjectData::Array { elements, .. }, ObjectData::Integer(i)) => {
                        if *i < 0 || *i as usize >= elements.len() {
                            eprintln!("❌ ERROR: Index out of bounds");
                            for frame in self.call_stack.iter().rev() {
                                eprintln!("    called from '{}' [line {}:{}]", frame.name, frame.line, frame.column);
                            }
                            eprintln!();
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
                            None => {
                                eprintln!("❌ ERROR: Key '{}' not found in dict", search_key);
                                EvalResult::Error
                            }
                        }
                    }
                    _ => {
                        eprintln!("❌ ERROR: Index operator not supported for these types");
                        for frame in self.call_stack.iter().rev() {
                            eprintln!("    called from '{}' [line {}:{}]", frame.name, frame.line, frame.column);
                        }
                        eprintln!();
                        EvalResult::Error
                    }
                }
            }

            Expression::DictLiteral(dict_lit) => {
                let mut entries: Vec<(ObjectRef, ObjectRef)> = Vec::new();
                for (key_expr, val_expr) in &dict_lit.entries {
                    let key_ref = match self.eval_expression(key_expr) {
                        EvalResult::Value(r) => r,
                        _ => return EvalResult::Error,
                    };
                    let val_ref = match self.eval_expression(val_expr) {
                        EvalResult::Value(r) => r,
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
                let obj_ref = match self.eval_expression(&dot_call.object) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };

                let obj_data = match self.resolve(obj_ref) {
                    Some(d) => d.clone(),
                    None => {
                        eprintln!("❌ ERROR: Invalid reference in dot call");
                        return EvalResult::Error;
                    }
                };

                match obj_data {
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
                                            _ => return EvalResult::Error,
                                        };
                                        let v = match self.eval_expression(v_expr) {
                                            EvalResult::Value(r) => r,
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
                            "toList" => {
                                let keys: Vec<OwnedValue> = entries.iter()
                                    .map(|&(k, _)| self.extract(k))
                                    .collect();
                                let refs: Vec<ObjectRef> = keys.into_iter()
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

                            _ => {
                                eprintln!("❌ ERROR: Unknown dict method '{}'", dot_call.method);
                                EvalResult::Error
                            }
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
                }
            }

            Expression::Prefix(op, right_expr) => {
                let right_ref = match self.eval_expression(right_expr) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };
                let right_data = self.resolve(right_ref).unwrap().clone();
                self.eval_prefix(op, right_data)
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
                    _ => return EvalResult::Error,
                };
                let right_ref = match self.eval_expression(&infix_expr.right) {
                    EvalResult::Value(r) => r,
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

    fn eval_prefix(&mut self, op: &str, right: ObjectData) -> EvalResult {
        match op {
            "-" => match right {
                ObjectData::Integer(i) => EvalResult::Value(self.alloc(ObjectData::Integer(-i))),
                ObjectData::Decimal(d) => EvalResult::Value(self.alloc(ObjectData::Decimal(-d))),
                _ => {
                    eprintln!("❌ ERROR: Prefix '-' not supported for this type");
                    EvalResult::Error
                }
            },
            "!" => {
                if let ObjectData::Boolean(b) = right {
                    EvalResult::Value(self.alloc(ObjectData::Boolean(!b)))
                } else {
                    eprintln!("❌ ERROR: Prefix '!' only applies to booleans");
                    EvalResult::Error
                }
            }
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
                    "+" => ObjectData::Str(l + &r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
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
                    _ => {
                        eprintln!(
                            "❌ ERROR: Operator '{}' not supported between String and Integer",
                            op
                        );
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            _ => {
                eprint!(
                    "❌ ERROR: Type mismatch — operator '{}' cannot be applied between '{}' and '{}' - [{}:{}]",
                    op, left_type, right_type, line, column
                );
                for frame in self.call_stack.iter().rev() {
                    eprintln!(
                        "    called from '{}' [line {}:{}]",
                        frame.name, frame.line, frame.column
                    );
                }
                eprintln!();
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

    // ── Callback calling helper ───────────────────────────────────────────────

    fn call_function(&mut self, func_ref: ObjectRef, arg_vals: Vec<OwnedValue>) -> EvalResult {
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
                for (name, owned) in captured {
                    let r = self.plant(owned);
                    self.scopes.declare(name, r);
                }
                for (param, val) in parameters.iter().zip(arg_vals.into_iter()) {
                    let r = self.plant(val);
                    self.scopes.declare(param.name.clone(), r);
                }
                let mut result_ref = self.null_ref;
                for s in &body.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(v) => result_ref = v,
                        EvalResult::Return(v) => { result_ref = v; break; }
                        EvalResult::Error => { self.scopes.pop(); return EvalResult::Error; }
                    }
                }
                let owned = self.extract(result_ref);
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

    fn eval_parse_int(&mut self, args: &[ast::Expression]) -> EvalResult {
        if args.len() != 1 {
            eprintln!("❌ ERROR: parseInt expects 1 argument");
            return EvalResult::Error;
        }
        let r = match self.eval_expression(&args[0]) {
            EvalResult::Value(r) => r,
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
                    eprintln!("❌ ERROR: pop on empty array");
                    return EvalResult::Error;
                }
                let mut e = elems;
                let last = e.pop().unwrap();
                let owned = self.extract(last);
                self.update_array(arr_ref, element_type, e);
                EvalResult::Value(self.plant(owned))
            }

            "shift" => {
                if elems.is_empty() {
                    eprintln!("❌ ERROR: shift on empty array");
                    return EvalResult::Error;
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

            "sort" => {
                // If a function comparator is provided, use it
                let use_comparator = dot_call.arguments.len() == 1 && {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => matches!(self.resolve(r), Some(ObjectData::Function { .. })),
                        _ => false,
                    }
                };

                if use_comparator {
                    let cb_ref = match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => r,
                        _ => return EvalResult::Error,
                    };
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

                let order = if dot_call.arguments.is_empty() {
                    "asc".to_string()
                } else {
                    match self.eval_expression(&dot_call.arguments[0]) {
                        EvalResult::Value(r) => match self.resolve(r).cloned() {
                            Some(ObjectData::Str(s)) => s,
                            _ => "asc".to_string(),
                        },
                        _ => return EvalResult::Error,
                    }
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
                    _ => return EvalResult::Error,
                };
                let cb_ref = match self.eval_expression(&dot_call.arguments[1]) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };
                let owned_elems: Vec<OwnedValue> = elems.iter().map(|&r| self.extract(r)).collect();
                let mut acc_ref = init_ref;
                for val in owned_elems {
                    let acc_val = self.extract(acc_ref);
                    acc_ref = match self.call_function(cb_ref, vec![acc_val, val]) {
                        EvalResult::Value(r) => r,
                        _ => return EvalResult::Error,
                    };
                }
                EvalResult::Value(acc_ref)
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
                if dot_call.arguments.len() != 2 {
                    eprintln!("❌ ERROR: substring expects 2 arguments (start, end)");
                    return EvalResult::Error;
                }
                let start = match self.eval_int_arg(&dot_call.arguments[0]) { Some(v) => v, None => return EvalResult::Error };
                let end   = match self.eval_int_arg(&dot_call.arguments[1]) { Some(v) => v, None => return EvalResult::Error };
                let chars: Vec<char> = s.chars().collect();
                let len = chars.len() as i64;
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
}

// ── Free helpers ──────────────────────────────────────────────────────────────

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
        _ => false,
    }
}
