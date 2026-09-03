use crate::ast::{self, Expression, Program, Statement};
use crate::diagnostic::{Diagnostic, Phase};
use crate::render;
use crate::span::Span;
use std::collections::HashMap;

/// Generic semantic diagnostic: a type error not yet given a narrower code.
///
/// The checker is deliberately partial — runtime checks stay authoritative —
/// so its findings are reported as advisory rather than fatal. Narrower codes
/// get split out as individual checks acquire tests that pin their meaning.
pub const SZ_TYPE_ERROR: &str = "SZ3000";

/// A type finding, collected so tools (the LSP) can map it to a range while
/// the CLI renders it to stderr.
///
/// M3 collapsed this into the shared [`Diagnostic`]. It carries
/// `Severity::Advisory`, which is not a detail: `spec/types.md` states the
/// checker is deliberately partial and that `sz file.sz` reports its findings
/// **and still runs**. A position of `0` means unknown and renders as no
/// position at all rather than as `line 0:0`.
pub type TypeError = Diagnostic;

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
        self.type_error_code(SZ_TYPE_ERROR, line, column, message);
    }

    /// Report a type error under a specific stable diagnostic code.
    fn type_error_code(&self, code: &'static str, line: usize, column: usize, message: String) {
        let diagnostic = Diagnostic::frontend(
            code,
            Phase::Type,
            // `0` is the checker's "unknown position"; `Span::point(0, 0)` is
            // `Span::unknown()`, so the two spellings already agree, and the
            // renderer drops the bracket rather than printing `line 0:0`.
            Span::point(line, column),
            message,
        );
        // No source name and no source lines: the checker is never handed
        // either, which is why its diagnostics say `[line L:C]` even when `sz`
        // was given a path, and carry no caret. Recorded as an inconsistency in
        // `docs/maturity/ROADMAP_STATE.md`, preserved here.
        eprintln!(
            "{}",
            render::render(&diagnostic, &render::Context::default())
        );
        self.errors.borrow_mut().push(diagnostic);
    }

    pub fn check(&mut self) {
        let stmts = &self.program.statements;

        // Pass 1: collect all function declarations
        for stmt in stmts {
            if let Statement::FunctionDeclaration(f) = unwrap_export(stmt) {
                self.functions.insert(f.name.clone(), f.function.clone());
            }
        }

        // Pass 2: infer types for top-level let bindings
        for stmt in stmts {
            if let Statement::Let(l) = unwrap_export(stmt) {
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
            Expression::Integer { value: _, .. } => Some("int".to_string()),
            Expression::Decimal { value: _, .. } => Some("decimal".to_string()),
            Expression::Dec { value: _, .. } => Some("dec".to_string()),
            Expression::String { value: _, .. }
            | Expression::InterpolatedString { parts: _, .. } => Some("string".to_string()),
            Expression::Boolean { value: _, .. } => Some("bool".to_string()),
            Expression::Null { .. } => Some("null".to_string()),
            Expression::Identifier { name, .. } => self.var_types.get(name).cloned(),
            Expression::Call(call) => {
                if let Expression::Identifier { name: fname, .. } = call.function.as_ref() {
                    self.functions
                        .get(fname)
                        .and_then(|f| f.return_type.clone())
                } else {
                    None
                }
            }
            Expression::ArrayLiteral(arr) => arr.element_type.as_ref().map(|t| format!("[{}]", t)),
            Expression::If(if_expr) => {
                // Infer from consequence branch
                if_expr.consequence.statements.last().and_then(|s| {
                    if let Statement::Expression(e) = s {
                        self.infer_type(e)
                    } else {
                        None
                    }
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
            Statement::NestedFieldAssign(_) => {}
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
                self.type_error(
                    0,
                    0,
                    format!(
                        "Array declared as [{}] but contains element of type '{}'.",
                        element_type, actual
                    ),
                );
            }
        }
    }

    // ── Call checking ─────────────────────────────────────────────────────────

    fn check_call(&self, call: &ast::CallExpression) {
        let func_name = match call.function.as_ref() {
            Expression::Identifier { name: n, .. } => n,
            _ => return,
        };

        let func = match self.functions.get(func_name) {
            Some(f) => f,
            None => return,
        };

        // Skip arity check if any argument is a spread expression
        let has_spread_arg = call
            .arguments
            .iter()
            .any(|a| matches!(a, Expression::Spread { value: _, .. }));
        if has_spread_arg {
            return;
        }

        let has_rest = func.parameters.last().map(|p| p.is_rest).unwrap_or(false);
        let required_count = func
            .parameters
            .iter()
            .filter(|p| !p.is_rest && p.default_value.is_none())
            .count();
        let min_params = required_count;
        let max_params = if has_rest {
            usize::MAX
        } else {
            func.parameters.len()
        };
        let arity_ok = call.arguments.len() >= min_params && call.arguments.len() <= max_params;
        if !arity_ok {
            let expected_str = if has_rest {
                format!("at least {}", min_params)
            } else if min_params == max_params {
                format!("{}", min_params)
            } else {
                format!("{}-{}", min_params, max_params)
            };
            self.type_error(
                call.span.line,
                call.span.column,
                format!(
                    "'{}' expects {} argument(s) but got {}.",
                    func_name,
                    expected_str,
                    call.arguments.len()
                ),
            );
            return;
        }

        for (i, param) in func.parameters.iter().enumerate() {
            if i >= call.arguments.len() {
                break;
            }
            let expected = match &param.type_name {
                Some(t) => t,
                None => continue,
            };

            let actual = match self.infer_type(&call.arguments[i]) {
                Some(t) => t,
                None => continue,
            };

            if !types_compatible(expected, &actual) {
                self.type_error(
                    call.span.line,
                    call.span.column,
                    format!(
                        "Parameter '{}' of '{}' expected '{}' but received '{}'.",
                        param.name, func_name, expected, actual
                    ),
                );
            }
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Does a value whose inferred type is named `actual` satisfy a declared
/// `expected`?
///
/// This is the **name-level** half of the rule `evaluator::type_matches`
/// implements over values, and `spec/types.md` is normative for both. Where the
/// two disagreed, the checker was the one at fault every time: it is advisory,
/// so a finding on a program that runs correctly is noise printed over correct
/// code, and noise is how a linter teaches people to ignore it.
///
/// `tests/type_agreement.rs` holds the two matchers to each other, case by case.
///
/// What it still cannot express, and why that is not a divergence: the checker
/// reasons about type *names*, so the arms of `type_matches` that inspect a
/// value — a class instance's name, an enum variant's enum, a `DateField`
/// behaving as an `int` — have no name-level counterpart. `infer_type` never
/// produces those names either, so the two never actually meet on them.
/// Look through `export` to the declaration it wraps.
///
/// `export` changes a declaration's visibility to other modules and nothing
/// about what it declares, and `spec/types.md` never mentions it. Passes 1 and 2
/// used to match on `Statement` directly, so an exported function never entered
/// `self.functions` and an exported `let` never entered `self.var_types` — while
/// pass 3, which already unwrapped, checked their *bodies* normally. The result
/// was that `f("hello")` against `fn int f(int a)` was caught statically, and the
/// same program with `export` in front of it only at run time.
///
/// A loop rather than one unwrap: nothing in the grammar produces nested
/// `export`, and a `while` costs nothing to be right if that ever changes.
fn unwrap_export(statement: &Statement) -> &Statement {
    let mut current = statement;
    while let Statement::Export(inner) = current {
        current = inner;
    }
    current
}

fn types_compatible(expected: &str, actual: &str) -> bool {
    if expected == actual || expected == "any" {
        return true;
    }
    // `void` accepts `null`. `spec/types.md` lists this in the matching table
    // and `type_matches` implements it; the checker used to report
    // `fn void f() { return null; }` — the most ordinary way to write a void
    // function — as a return-type mismatch.
    if expected == "void" && actual == "null" {
        return true;
    }
    // A `[T]` parameter accepts **any** array, whatever its elements, and so
    // does `array`. Both are `spec/types.md`'s wording and `type_matches`'s
    // behaviour; the checker used to compare the two annotations as strings, so
    // `[string]` at a `[int]` parameter was reported even though it runs.
    //
    // This does not loosen `check_array_literal`, which is the check that gives
    // `[T]` its meaning: there, `expected` is the element type — a keyword —
    // and never an array annotation.
    if is_array_type(expected) && is_array_type(actual) {
        return true;
    }
    // Nullable: "int?" accepts "int" or "null"
    if let Some(base) = expected.strip_suffix('?') {
        return actual == base || actual == "null";
    }
    false
}

/// Whether a type name denotes "an array", in either spelling the language has.
fn is_array_type(name: &str) -> bool {
    name == "array" || (name.starts_with('[') && name.ends_with(']'))
}
