use crate::ast::{self, Expression, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, RegionId};
use crate::scope::ScopeStack;
use std::collections::HashMap;

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),  // Ejecución normal (retorno implícito)
    Return(ObjectRef), // Ejecución interrumpida por `return`
    Error,             // Ocurrió un error
}

// ── OwnedValue ────────────────────────────────────────────────────────────────
// Árbol de valores completamente dueño de sus datos, sin refs a ninguna arena.
// Permite extraer un resultado ANTES de truncar un scope y re-alocarlo DESPUÉS,
// resolviendo el problema de refs colgantes en arrays devueltos por funciones.
#[derive(Clone)]
enum OwnedValue {
    Integer(i64),
    Boolean(bool),
    Str(String),
    Array(Vec<OwnedValue>),
    Function {
        return_type: Option<String>,
        parameters: Vec<ast::Parameter>,
        body: ast::BlockStatement,
    },
    Null,
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

    pub fn display(&self, obj_ref: ObjectRef) -> String {
        match self.resolve(obj_ref) {
            Some(ObjectData::Integer(i)) => format!("{}", i),
            Some(ObjectData::Boolean(b)) => format!("{}", b),
            Some(ObjectData::Str(s)) => format!("{}", s),
            Some(ObjectData::Array(refs)) => {
                let elems: Vec<String> = refs.iter().map(|&r| self.display(r)).collect();
                format!("[{}]", elems.join(", "))
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
            Some(ObjectData::Boolean(b)) => OwnedValue::Boolean(*b),
            Some(ObjectData::Str(s)) => OwnedValue::Str(s.clone()),
            Some(ObjectData::Array(refs)) => {
                OwnedValue::Array(refs.iter().map(|&r| self.extract(r)).collect())
            }
            Some(ObjectData::Function {
                return_type,
                parameters,
                body,
            }) => OwnedValue::Function {
                return_type: return_type.clone(),
                parameters: parameters.clone(),
                body: body.clone(),
            },
            Some(ObjectData::Null) | None => OwnedValue::Null,
        }
    }

    // Re-aloca un OwnedValue en la arena activa (scope padre o global).
    // Debe llamarse DESPUÉS de scopes.pop().
    fn plant(&mut self, value: OwnedValue) -> ObjectRef {
        match value {
            OwnedValue::Integer(i) => self.alloc(ObjectData::Integer(i)),
            OwnedValue::Boolean(b) => self.alloc(ObjectData::Boolean(b)),
            OwnedValue::Str(s) => self.alloc(ObjectData::Str(s)),
            OwnedValue::Array(items) => {
                let refs: Vec<ObjectRef> = items.into_iter().map(|v| self.plant(v)).collect();
                self.alloc(ObjectData::Array(refs))
            }
            OwnedValue::Function {
                return_type,
                parameters,
                body,
            } => self.alloc(ObjectData::Function {
                return_type,
                parameters,
                body,
            }),
            OwnedValue::Null => self.null_ref,
        }
    }

    // ── Evaluación de Programa ──────────────────────────────────────────────

    pub fn eval_program(&mut self, program: &Program) -> Option<ObjectRef> {
        let mut result = self.null_ref;
        for statement in &program.statements {
            // Out and pure Expression statements at top level produce values that are
            // immediately consumed and never retained. Use a scratch watermark so
            // those temporaries don't accumulate in the global arena for the program lifetime.
            let scratch_mark = match statement {
                Statement::Out(_) | Statement::Expression(_) => {
                    Some(self.global_arena.watermark())
                }
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
                let func_data = ObjectData::Function {
                    return_type: func_decl.function.return_type.clone(),
                    parameters: func_decl.function.parameters.clone(),
                    body: func_decl.function.body.clone(),
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
                let mut result = EvalResult::Value(self.null_ref);
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

                    // 2. Evaluate body block without throwing Flash Scope Error on return
                    match self.eval_block(&while_stmt.body) {
                        EvalResult::Value(v) => result = EvalResult::Value(v),
                        EvalResult::Return(v) => {
                            // Un return dentro de un while interrumpe el while y sube el return
                            return EvalResult::Return(v);
                        }
                        EvalResult::Error => return EvalResult::Error,
                    }
                }
                result
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
            Expression::String(s) => EvalResult::Value(self.alloc(ObjectData::Str(s.clone()))),
            Expression::Boolean(b) => EvalResult::Value(self.alloc(ObjectData::Boolean(*b))),

            Expression::Identifier(name) => match self.lookup_var(name) {
                Some(r) => EvalResult::Value(r),
                None => {
                    eprintln!("❌ ERROR: Variable not found: {}", name);
                    EvalResult::Error
                }
            },

            Expression::FunctionLiteral(func_lit) => {
                let func_data = ObjectData::Function {
                    return_type: func_lit.return_type.clone(),
                    parameters: func_lit.parameters.clone(),
                    body: func_lit.body.clone(),
                };
                EvalResult::Value(self.alloc(func_data))
            }

            Expression::Call(call_expr) => {
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
                let (return_type, parameters, body) = match func_data {
                    Some(ObjectData::Function {
                        return_type,
                        parameters,
                        body,
                    }) => (return_type, parameters, body),
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
                        let is_valid = match (expected_type.as_str(), actual_data) {
                            ("int", ObjectData::Integer(_)) => true,
                            ("string", ObjectData::Str(_)) => true,
                            ("bool", ObjectData::Boolean(_)) => true,
                            _ => false,
                        };

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
                    let is_valid = match (expected_ret.as_str(), actual_data) {
                        ("int", ObjectData::Integer(_)) => true,
                        ("string", ObjectData::Str(_)) => true,
                        ("bool", ObjectData::Boolean(_)) => true,
                        ("void", ObjectData::Null) => true,
                        _ => false,
                    };
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

            Expression::ArrayLiteral(elements) => {
                let mut refs = Vec::new();
                for el in elements {
                    match self.eval_expression(el) {
                        EvalResult::Value(r) => refs.push(r),
                        _ => return EvalResult::Error,
                    }
                }
                EvalResult::Value(self.alloc(ObjectData::Array(refs)))
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

                if let (ObjectData::Array(elements), ObjectData::Integer(i)) = (left_data, idx_data)
                {
                    if i < 0 || i as usize >= elements.len() {
                        eprintln!("❌ ERROR: Index out of bounds");

                        for frame in self.call_stack.iter().rev() {
                            eprintln!(
                                "    called from '{}' [line {}:{}]",
                                frame.name, frame.line, frame.column
                            );
                        }
                        eprintln!();
                        EvalResult::Error
                    } else {
                        EvalResult::Value(elements[i as usize])
                    }
                } else {
                    eprintln!("❌ ERROR: Index operator not supported for these types");

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
            "-" => {
                if let ObjectData::Integer(i) = right {
                    EvalResult::Value(self.alloc(ObjectData::Integer(-i)))
                } else {
                    eprintln!("❌ ERROR: Prefix '-' not supported for this type");
                    EvalResult::Error
                }
            }
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
            ast::Expression::Boolean(_) => 1,
            ast::Expression::String(s) => 24 + s.len(), // Rust String overhead + capacity
            ast::Expression::Identifier(_) => 8,        // reference resolution
            ast::Expression::Prefix(_, right) => 8 + self.estimate_expression(right),
            ast::Expression::Infix(infix) => {
                8 + self.estimate_expression(&infix.left) + self.estimate_expression(&infix.right)
            }
            ast::Expression::FunctionLiteral(f) => {
                // A closure allocation is roughly size of its context + struct size
                32 + f.parameters.len() * 8
            }
            ast::Expression::Call(c) => {
                let mut cost = 8; // function call overhead
                for arg in &c.arguments {
                    cost += self.estimate_expression(arg);
                }
                cost
            }
            ast::Expression::ArrayLiteral(arr) => {
                let mut cost = 24; // Vec overhead
                for item in arr {
                    cost += self.estimate_expression(item);
                }
                cost
            }
            ast::Expression::If(if_expr) => {
                let mut cost = self.estimate_expression(&if_expr.condition);
                // Pessimistically assume the largest branch is taken
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
        }
    }
}
