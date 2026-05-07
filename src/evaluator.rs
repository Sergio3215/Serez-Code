use crate::ast::{self, Expression, Program, Statement};
use crate::region::{Arena, ObjectData, ObjectRef, RegionId};
use crate::scope::ScopeStack;
use std::collections::HashMap;

// ── EvalResult ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum EvalResult {
    Value(ObjectRef),   // Ejecución normal (retorno implícito)
    Return(ObjectRef),  // Ejecución interrumpida por `return`
    Error,              // Ocurrió un error
}

// ── Evaluator ─────────────────────────────────────────────────────────────────

pub struct Evaluator {
    global_arena:    Arena,
    global_bindings: HashMap<String, ObjectRef>,
    scopes:          ScopeStack,
    null_ref:        ObjectRef,
}

impl Evaluator {
    pub fn new() -> Self {
        let mut global_arena = Arena::new();
        let null_idx = global_arena.alloc(ObjectData::Null);
        let null_ref = ObjectRef { region: RegionId::Global, index: null_idx };

        Evaluator {
            global_arena,
            global_bindings: HashMap::new(),
            scopes: ScopeStack::new(),
            null_ref,
        }
    }

    fn alloc(&mut self, data: ObjectData) -> ObjectRef {
        if self.scopes.is_empty() {
            let idx = self.global_arena.alloc(data);
            ObjectRef { region: RegionId::Global, index: idx }
        } else {
            let idx = self.scopes.arena.alloc(data);
            ObjectRef { region: RegionId::Scoped, index: idx }
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
            Some(ObjectData::Integer(i))  => format!("Integer({})", i),
            Some(ObjectData::Boolean(b))  => format!("Boolean({})", b),
            Some(ObjectData::Str(s))      => format!("String(\"{}\")", s),
            Some(ObjectData::Array(refs)) => {
                let elems: Vec<String> = refs.iter()
                    .map(|&r| self.display(r))
                    .collect();
                format!("Array([{}])", elems.join(", "))
            }
            Some(ObjectData::Function{..}) => "Function".to_string(),
            Some(ObjectData::Null) => "Null".to_string(),
            None => "❌ Referencia inválida".to_string(),
        }
    }

    // ── Evaluación de Programa ──────────────────────────────────────────────

    pub fn eval_program(&mut self, program: &Program) -> Option<ObjectRef> {
        let mut result = self.null_ref;
        for statement in &program.statements {
            match self.eval_statement(statement) {
                EvalResult::Value(v) => result = v,
                EvalResult::Return(_) => {
                    println!("❌ FLASH SCOPE ERROR: 'return' can only be used inside functions. Use 'export' for the global level.");
                    return None;
                }
                EvalResult::Error => return None,
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
                    println!("❌ ERROR: Undeclared variable: {}", assign_stmt.name);
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
                    self.global_bindings.insert(func_decl.name.clone(), func_ref);
                } else {
                    self.scopes.declare(func_decl.name.clone(), func_ref);
                }
                EvalResult::Value(self.null_ref)
            }

            Statement::Block(block_stmt) => {
                self.scopes.push();
                let mut result = EvalResult::Value(self.null_ref);

                for s in &block_stmt.statements {
                    match self.eval_statement(s) {
                        EvalResult::Value(v) => result = EvalResult::Value(v),
                        EvalResult::Return(v) => {
                            result = EvalResult::Return(v);
                            break; // Flash Scope: unwinding
                        }
                        EvalResult::Error => {
                            result = EvalResult::Error;
                            break;
                        }
                    }
                } // <--- Added missing brace!

                let result_data_opt = match &result {
                    EvalResult::Value(v) | EvalResult::Return(v) => self.resolve(*v).cloned(),
                    EvalResult::Error => None,
                };

                self.scopes.pop();

                if let Some(data) = result_data_opt {
                    let promoted_ref = self.alloc(data);
                    match result {
                        EvalResult::Value(_) => EvalResult::Value(promoted_ref),
                        EvalResult::Return(_) => EvalResult::Return(promoted_ref),
                        EvalResult::Error => EvalResult::Error,
                    }
                } else {
                    EvalResult::Error
                }
            }

            Statement::Return(return_stmt) => {
                match self.eval_expression(&return_stmt.return_value) {
                    EvalResult::Value(v) => EvalResult::Return(v),
                    _ => EvalResult::Error,
                }
            }

            Statement::Expression(expr) => self.eval_expression(expr),
        }
    }

    fn eval_expression(&mut self, expr: &Expression) -> EvalResult {
        match expr {
            Expression::Integer(i)  => EvalResult::Value(self.alloc(ObjectData::Integer(*i))),
            Expression::String(s)   => EvalResult::Value(self.alloc(ObjectData::Str(s.clone()))),
            Expression::Boolean(b)  => EvalResult::Value(self.alloc(ObjectData::Boolean(*b))),

            Expression::Identifier(name) => {
                match self.lookup_var(name) {
                    Some(r) => EvalResult::Value(r),
                    None => {
                        println!("❌ ERROR: Variable not found: {}", name);
                        EvalResult::Error
                    }
                }
            }

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

                let func_data = self.resolve(func_ref).cloned();
                let (return_type, parameters, body) = match func_data {
                    Some(ObjectData::Function { return_type, parameters, body }) => {
                        (return_type, parameters, body)
                    }
                    _ => {
                        println!("❌ ERROR: Attempt to call a non-function");
                        return EvalResult::Error;
                    }
                };

                let mut arg_refs = Vec::new();
                for arg in &call_expr.arguments {
                    match self.eval_expression(arg) {
                        EvalResult::Value(r) => arg_refs.push(r),
                        _ => return EvalResult::Error,
                    }
                }

                if arg_refs.len() != parameters.len() {
                    println!("❌ ERROR: Function expected {} arguments, got {}", parameters.len(), arg_refs.len());
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
                            println!("❌ TYPE ERROR: Parameter '{}' expected '{}' but received another type.", param.name, expected_type);
                            return EvalResult::Error;
                        }
                    }
                }

                self.scopes.push();

                for (i, param) in parameters.iter().enumerate() {
                    self.scopes.declare(param.name.clone(), arg_refs[i]);
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
                            return EvalResult::Error;
                        }
                    }
                } // <--- Added missing brace

                // Extraer el valor antes de destruir el Flash Scope
                let result_data_opt = self.resolve(result_ref).cloned();

                self.scopes.pop(); // Destrucción instantánea de temporales (Flash Scope)

                // Promovemos el resultado al scope actual (padre) o global
                let result_ref = if let Some(data) = result_data_opt {
                    self.alloc(data)
                } else {
                    self.null_ref
                };

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
                        println!("❌ TYPE ERROR: Function expected to return '{}' but returned another type.", expected_ret);
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

            Expression::Prefix(op, right_expr) => {
                let right_ref = match self.eval_expression(right_expr) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };
                let right_data = self.resolve(right_ref).unwrap().clone();
                self.eval_prefix(op, right_data)
            }

            Expression::Infix(left_expr, op, right_expr) => {
                let left_ref = match self.eval_expression(left_expr) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };
                let right_ref = match self.eval_expression(right_expr) {
                    EvalResult::Value(r) => r,
                    _ => return EvalResult::Error,
                };
                let left_data  = self.resolve(left_ref).unwrap().clone();
                let right_data = self.resolve(right_ref).unwrap().clone();
                self.eval_infix(op, left_data, right_data)
            }
        }
    }

    fn eval_prefix(&mut self, op: &str, right: ObjectData) -> EvalResult {
        match op {
            "-" => {
                if let ObjectData::Integer(i) = right {
                    EvalResult::Value(self.alloc(ObjectData::Integer(-i)))
                } else {
                    println!("❌ ERROR: Prefix '-' not supported for this type");
                    EvalResult::Error
                }
            }
            "!" => {
                if let ObjectData::Boolean(b) = right {
                    EvalResult::Value(self.alloc(ObjectData::Boolean(!b)))
                } else {
                    println!("❌ ERROR: Prefix '!' only applies to booleans");
                    EvalResult::Error
                }
            }
            _ => EvalResult::Error,
        }
    }

    fn eval_infix(&mut self, op: &str, left: ObjectData, right: ObjectData) -> EvalResult {
        match (left, right) {
            (ObjectData::Integer(l), ObjectData::Integer(r)) => {
                let result = match op {
                    "+"  => ObjectData::Integer(l + r),
                    "-"  => ObjectData::Integer(l - r),
                    "*"  => ObjectData::Integer(l * r),
                    "/"  => {
                        if r == 0 {
                            println!("❌ ERROR: Division by zero");
                            return EvalResult::Error;
                        }
                        ObjectData::Integer(l / r)
                    }
                    "%"  => ObjectData::Integer(l % r),
                    "<"  => ObjectData::Boolean(l < r),
                    ">"  => ObjectData::Boolean(l > r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => {
                        println!("❌ ERROR: Unknown operator: {}", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(l), ObjectData::Str(r)) => {
                let result = match op {
                    "+"  => ObjectData::Str(l + &r),
                    "==" => ObjectData::Boolean(l == r),
                    "!=" => ObjectData::Boolean(l != r),
                    _ => {
                        println!("❌ ERROR: Operator '{}' not supported between strings", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            (ObjectData::Str(s), ObjectData::Integer(n)) => {
                let result = match op {
                    "*" => {
                        if n < 0 {
                            println!("❌ ERROR: Cannot repeat a string with a negative n");
                            return EvalResult::Error;
                        }
                        ObjectData::Str(s.repeat(n as usize))
                    }
                    _ => {
                        println!("❌ ERROR: Operator '{}' not supported between String and Integer", op);
                        return EvalResult::Error;
                    }
                };
                EvalResult::Value(self.alloc(result))
            }
            _ => {
                println!("❌ ERROR: Type mismatch for this operation");
                EvalResult::Error
            }
        }
    }

    pub fn check_program(&self, program: &ast::Program) {
        println!("🚀 Starting static analysis (Flash Scope Criticality)...");
        println!("⚠️  NOTE: Cost in bytes is an estimated value based on AST heuristics, not an exact runtime measurement.\n");
        
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
            ast::Expression::Identifier(_) => 8, // reference resolution
            ast::Expression::Prefix(_, right) => 8 + self.estimate_expression(right),
            ast::Expression::Infix(left, _, right) => {
                8 + self.estimate_expression(left) + self.estimate_expression(right)
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
        }
    }
}
