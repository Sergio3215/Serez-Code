use crate::ast::{self, Expression, Program, Statement};
use std::collections::HashMap;

pub struct TypeChecker {
    program: Program,
    functions: HashMap<String, ast::FunctionLiteral>,
    var_types: HashMap<String, String>,
}

impl TypeChecker {
    pub fn new(program: Program) -> Self {
        TypeChecker {
            program,
            functions: HashMap::new(),
            var_types: HashMap::new(),
        }
    }

    pub fn check(&mut self) {
        for statement in &self.program.statements.clone() {
            if let Statement::FunctionDeclaration(func_decl) = statement {
                self.functions
                    .insert(func_decl.name.clone(), func_decl.function.clone());
            }
        }

        // Infer types for let-bound variables with literal RHS
        for statement in &self.program.statements.clone() {
            if let Statement::Let(let_stmt) = statement {
                let inferred = match &let_stmt.value {
                    Expression::Integer(_) => Some("int"),
                    Expression::Decimal(_) => Some("decimal"),
                    Expression::String(_) => Some("string"),
                    Expression::Boolean(_) => Some("bool"),
                    _ => None,
                };
                if let Some(t) = inferred {
                    self.var_types.insert(let_stmt.name.clone(), t.to_string());
                }
            }
        }

        for statement in &self.program.statements.clone() {
            self.check_statement(statement);
        }
    }

    fn check_statement(&self, statement: &Statement) {
        match statement {
            Statement::Let(let_stmt) => self.check_expression(&let_stmt.value),
            Statement::FunctionDeclaration(func_decl) => {
                for stmt in &func_decl.function.body.statements {
                    self.check_statement(stmt);
                }
            }
            _ => (),
        }
    }

    fn check_expression(&self, expression: &Expression) {
        match expression {
            Expression::Call(call_expr) => self.check_call(call_expr),
            _ => (),
        }
    }

    fn check_call(&self, call_expr: &ast::CallExpression) {
        // Resolver el nombre de la función
        let func_name = match call_expr.function.as_ref() {
            Expression::Identifier(name) => name,
            _ => return,
        };

        // Buscarla en la tabla
        let func = match self.functions.get(func_name) {
            Some(f) => f,
            None => return,
        };

        // Chequear cantidad de argumentos
        if call_expr.arguments.len() != func.parameters.len() {
            eprintln!(
                "❌ TYPE ERROR: '{}' expects {} arguments but got {}.",
                func_name,
                func.parameters.len(),
                call_expr.arguments.len()
            );
            return;
        }

        // Chequear tipo de cada argumento
        for (i, param) in func.parameters.iter().enumerate() {
            let expected_type = match &param.type_name {
                Some(t) => t,
                None => continue, // sin anotación → any → saltar
            };

            let actual_type = match &call_expr.arguments[i] {
                Expression::Integer(_) => "int",
                Expression::Decimal(_) => "decimal",
                Expression::String(_) => "string",
                Expression::Boolean(_) => "bool",
                Expression::Identifier(name) => match self.var_types.get(name) {
                    Some(t) => t.as_str(),
                    None => continue, // type unknown at static analysis time
                },
                _ => continue, // complex expression → skip
            };

            if actual_type != expected_type.as_str() {
                eprintln!(
                    "❌ TYPE ERROR [line {}:{}]: Parameter '{}' of '{}' expected '{}' but received '{}'.",
                    call_expr.line,
                    call_expr.column,
                    param.name,
                    func_name,
                    expected_type,
                    actual_type
                );
            }
        }
    }
}
