use crate::ast::{self, Expression, Program, Statement};
use std::collections::HashMap;

/// A type error with its source position (1-based; 0 = unknown position), as
/// reported alongside the stderr message. Collected so tools (LSP) can map
/// errors to ranges; the CLI keeps using stderr.
#[derive(Debug, Clone)]
pub struct TypeError {
    pub line: usize,
    pub column: usize,
    pub message: String,
}

pub struct TypeChecker<'a> {
    program: &'a Program,
    functions: HashMap<String, ast::FunctionLiteral>,
    var_types: HashMap<String, String>,
    /// Every error reported by `type_error`, in order. `RefCell` because the
    /// check methods take `&self`.
    errors: std::cell::RefCell<Vec<TypeError>>,
}

impl<'a> TypeChecker<'a> {
    pub fn new(program: &'a Program) -> Self {
        TypeChecker {
            program,
            functions: HashMap::new(),
            var_types: HashMap::new(),
            errors: std::cell::RefCell::new(Vec::new()),
        }
    }

    /// All type errors reported by `check`, with positions where known.
    pub fn take_errors(&self) -> Vec<TypeError> {
        self.errors.borrow().clone()
    }

    /// Report a type error: stderr (CLI behavior, unchanged) + collected list.
    fn type_error(&self, line: usize, column: usize, message: String) {
        eprintln!("❌ TYPE ERROR{}: {}",
            if line > 0 { format!(" [line {}:{}]", line, column) } else { String::new() },
            message);
        self.errors.borrow_mut().push(TypeError { line, column, message });
    }

    pub fn check(&mut self) {
        let stmts = &self.program.statements;

        // Pass 1: collect all function declarations
        for stmt in stmts {
            if let Statement::FunctionDeclaration(f) = stmt {
                self.functions.insert(f.name.clone(), f.function.clone());
            }
        }

        // Pass 2: infer types for top-level let bindings
        for stmt in stmts {
            if let Statement::Let(l) = stmt {
                if let Some(t) = self.infer_type(&l.value) {
                    self.var_types.insert(l.name.clone(), t);
                }
            }
        }

        // Pass 3: full type checking
        for stmt in stmts {
            self.check_statement(stmt, None);
        }
    }

    // ── Type inference ────────────────────────────────────────────────────────

    fn infer_type(&self, expr: &Expression) -> Option<String> {
        match expr {
            Expression::Integer(_) => Some("int".to_string()),
            Expression::Decimal(_) => Some("decimal".to_string()),
            Expression::Dec(_) => Some("dec".to_string()),
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
                            self.type_error(0, 0, format!(
                                "Function declares return '{}' but 'return' expression has type '{}'.",
                                expected, actual
                            ));
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
            Statement::While(w) | Statement::DoWhile(w) => {
                for s in &w.body.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::For(f) => {
                for s in &f.body.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::ForEach(fe) => {
                self.check_expression(&fe.iterable, expected_return);
                for s in &fe.body.statements {
                    self.check_statement(s, expected_return);
                }
            }
            Statement::Block(b) | Statement::Unsafe(b) => {
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
            Statement::ClassDeclaration(_) => {}
            Statement::InterfaceDeclaration(_) => {}
            Statement::FieldAssign(_) => {}
            Statement::Break => {}
            Statement::Continue => {}
            Statement::BreakLabel(_) => {}
            Statement::ContinueLabel(_) => {}
            Statement::EnumDeclaration(_) => {}
            Statement::Throw(e) => {
                self.check_expression(e, expected_return);
            }
            Statement::Switch(sw) => {
                self.check_expression(&sw.value, expected_return);
                for case in &sw.cases {
                    for v in &case.values {
                        self.check_expression(v, expected_return);
                    }
                    for s in &case.body.statements {
                        self.check_statement(s, expected_return);
                    }
                }
                if let Some(ref d) = sw.default {
                    for s in &d.statements {
                        self.check_statement(s, expected_return);
                    }
                }
            }
            Statement::DerefAssign { ptr, value } => {
                self.check_expression(ptr, expected_return);
                self.check_expression(value, expected_return);
            }
            Statement::NativeDeclaration(_) => {}
            Statement::Import(_) => {}
            Statement::UsePermissions(_) => {}
            Statement::Export(inner) => self.check_statement(inner, expected_return),
            Statement::LetDestructureArray(d) => self.check_expression(&d.value, expected_return),
            Statement::LetDestructureDict(d) => self.check_expression(&d.value, expected_return),
            Statement::Yield(expr) => self.check_expression(expr, expected_return),
            Statement::Try(t) => {
                for s in &t.body.statements {
                    self.check_statement(s, expected_return);
                }
                if let Some(ref cb) = t.catch_body {
                    for s in &cb.statements {
                        self.check_statement(s, expected_return);
                    }
                }
                if let Some(ref fb) = t.finally_body {
                    for s in &fb.statements {
                        self.check_statement(s, expected_return);
                    }
                }
            }
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
            Expression::Ternary(t) => {
                self.check_expression(&t.condition, expected_return);
                self.check_expression(&t.then_expr, expected_return);
                self.check_expression(&t.else_expr, expected_return);
            }
            Expression::Match(m) => {
                self.check_expression(&m.subject, expected_return);
                for arm in &m.arms {
                    if let Some(g) = &arm.guard {
                        self.check_expression(g, expected_return);
                    }
                    for s in &arm.body.statements {
                        self.check_statement(s, expected_return);
                    }
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
                self.type_error(0, 0, format!(
                    "Array declared as [{}] but contains element of type '{}'.",
                    element_type, actual
                ));
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

        // Skip arity check if any argument is a spread expression
        let has_spread_arg = call.arguments.iter().any(|a| matches!(a, Expression::Spread(_)));
        if has_spread_arg { return; }

        let has_rest = func.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required_count = func.parameters.iter().filter(|p| !p.is_rest && p.default_value.is_none()).count();
        let min_params = required_count;
        let max_params = if has_rest { usize::MAX } else { func.parameters.len() };
        let arity_ok = call.arguments.len() >= min_params && call.arguments.len() <= max_params;
        if !arity_ok {
            let expected_str = if has_rest {
                format!("at least {}", min_params)
            } else if min_params == max_params {
                format!("{}", min_params)
            } else {
                format!("{}-{}", min_params, max_params)
            };
            self.type_error(call.line, call.column, format!(
                "'{}' expects {} argument(s) but got {}.",
                func_name, expected_str, call.arguments.len()
            ));
            return;
        }

        for (i, param) in func.parameters.iter().enumerate() {
            if i >= call.arguments.len() { break; }
            let expected = match &param.type_name {
                Some(t) => t,
                None => continue,
            };

            let actual = match self.infer_type(&call.arguments[i]) {
                Some(t) => t,
                None => continue,
            };

            if !types_compatible(expected, &actual) {
                self.type_error(call.line, call.column, format!(
                    "Parameter '{}' of '{}' expected '{}' but received '{}'.",
                    param.name, func_name,
                    expected, actual
                ));
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
