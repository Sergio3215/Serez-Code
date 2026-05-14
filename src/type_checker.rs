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
        // Pass 1: collect all function declarations into the lookup table
        for stmt in &self.program.statements.clone() {
            if let Statement::FunctionDeclaration(f) = stmt {
                self.functions.insert(f.name.clone(), f.function.clone());
            }
        }

        // Pass 2: infer types for all top-level let bindings
        for stmt in &self.program.statements.clone() {
            if let Statement::Let(l) = stmt {
                if let Some(t) = self.infer_type(&l.value) {
                    self.var_types.insert(l.name.clone(), t);
                }
            }
        }

        // Pass 3: full type checking
        for stmt in &self.program.statements.clone() {
            self.check_statement(stmt, None);
        }
    }

    // ── Type inference ────────────────────────────────────────────────────────

    fn infer_type(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Integer(_) => Some("int".to_string()),
            Expression::Decimal(_) => Some("decimal".to_string()),
            Expression::String(_) | Expression::InterpolatedString(_) => Some("string".to_string()),
            Expression::Boolean(_) => Some("bool".to_string()),
            Expression::Null => Some("null".to_string()),
            Expression::Identifier(name) => self.var_types.get(name).cloned(),
            Expression::Call(call) => {
                if let Expression::Identifier(fname) = call.function.as_ref() {
                    self.functions.get(fname).and_then(|f| f.return_type.clone())
                } else {
                    None
                }
            }
            Expression::ArrayLiteral(arr) => {
                arr.element_type.as_ref().map(|t| format!("[{}]", t))
            }
            Expression::If(if_expr) => {
                // Infer from consequence branch
                if_expr.consequence.statements.last().and_then(|s| {
                    if let Statement::Expression(e) = s { self.infer_type(e) } else { None }
                })
            }
            _ => None,
        }
    }

    // ── Statement checking ────────────────────────────────────────────────────

    fn check_statement(&self, stmt: &Statement, expected_return: Option<&str>) {
        match stmt {
            Statement::Let(l) => {
                self.check_expression(&l.value, expected_return);
            }
            Statement::Assign(_) => {}
            Statement::Return(ret) => {
                if let Some(expected) = expected_return {
                    if let Some(actual) = self.infer_type(&ret.return_value) {
                        if !types_compatible(expected, &actual) {
                            eprintln!(
                                "❌ TYPE ERROR: Function declares return '{}' but 'return' expression has type '{}'.",
                                expected, actual
                            );
                        }
                    }
                }
                self.check_expression(&ret.return_value, expected_return);
            }
            Statement::FunctionDeclaration(f) => {
                let ret = f.function.return_type.as_deref();
                for s in &f.function.body.statements {
                    self.check_statement(s, ret);
                }
            }
            Statement::While(w) => {
                for s in &w.body.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::For(f) => {
                for s in &f.body.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::Block(b) => {
                for s in &b.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::Out(o) => {
                self.check_expression(&o.value, expected_return);
            }
            Statement::Expression(e) => {
                self.check_expression(e, expected_return);
            }
            Statement::IndexAssign(_) => {}
        }
    }

    // ── Expression checking ───────────────────────────────────────────────────

    fn check_expression(&self, expr: &Expression, expected_return: Option<&str>) {
        match expr {
            Expression::Call(call) => self.check_call(call),
            Expression::ArrayLiteral(arr) => self.check_array_literal(arr),
            Expression::If(if_expr) => {
                for s in &if_expr.consequence.statements {
                    self.check_statement(s, expected_return);
                }
                if let Some(alt) = &if_expr.alternative {
                    for s in &alt.statements {
                        self.check_statement(s, expected_return);
                    }
                }
            }
            Expression::DotCall(dc) => {
                for arg in &dc.arguments {
                    self.check_expression(arg, expected_return);
                }
            }
            _ => {}
        }
    }

    // ── Array literal checking ────────────────────────────────────────────────

    fn check_array_literal(&self, arr: &ast::ArrayLiteral) {
        let element_type = match &arr.element_type {
            Some(t) => t,
            None => return,
        };
        for elem in &arr.elements {
            let actual = match self.infer_type(elem) {
                Some(t) => t,
                None => continue,
            };
            if !types_compatible(element_type, &actual) {
                eprintln!(
                    "❌ TYPE ERROR: Array declared as [{}] but contains element of type '{}'.",
                    element_type, actual
                );
            }
        }
    }

    // ── Call checking ─────────────────────────────────────────────────────────

    fn check_call(&self, call: &ast::CallExpression) {
        let func_name = match call.function.as_ref() {
            Expression::Identifier(n) => n,
            _ => return,
        };

        let func = match self.functions.get(func_name) {
            Some(f) => f,
            None => return,
        };

        if call.arguments.len() != func.parameters.len() {
            eprintln!(
                "❌ TYPE ERROR: '{}' expects {} argument(s) but got {}.",
                func_name,
                func.parameters.len(),
                call.arguments.len()
            );
            return;
        }

        for (i, param) in func.parameters.iter().enumerate() {
            let expected = match &param.type_name {
                Some(t) => t,
                None => continue,
            };

            let actual = match self.infer_type(&call.arguments[i]) {
                Some(t) => t,
                None => continue,
            };

            if !types_compatible(expected, &actual) {
                eprintln!(
                    "❌ TYPE ERROR [line {}:{}]: Parameter '{}' of '{}' expected '{}' but received '{}'.",
                    call.line, call.column,
                    param.name, func_name,
                    expected, actual
                );
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn types_compatible(expected: &str, actual: &str) -> bool {
    if expected == actual { return true; }
    if expected == "any" { return true; }
    // Nullable: "int?" accepts "int" or "null"
    if expected.ends_with('?') {
        let base = &expected[..expected.len() - 1];
        return actual == base || actual == "null";
    }
    false
}
