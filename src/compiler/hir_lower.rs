use crate::ast::{self, Expression, NewArgs, Statement, StringPart};
use crate::compiler::hir::*;
use crate::compiler::types::SzType;
/// AST → HIR lowering.
///
/// Walks the source AST and produces the High-level IR:
///   - Resolves types from annotations and inference
///   - Desugars complex constructs into simpler HIR forms
///   - Wraps top-level statements in an implicit `__sz_main` function
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompilerDiagnostic {
    pub code: &'static str,
    pub kind: &'static str,
    pub message: String,
}

impl std::fmt::Display for CompilerDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} {}: {}", self.code, self.kind, self.message)
    }
}

pub struct HirLowerer {
    /// Variable name → inferred compile-time type
    type_env: HashMap<String, SzType>,
    /// Function name → (param types, return type)
    fn_sigs: HashMap<String, (Vec<SzType>, SzType)>,
    /// Counter for generating unique synthetic variable names
    counter: usize,
    diagnostics: Vec<CompilerDiagnostic>,
}

impl HirLowerer {
    pub fn new() -> Self {
        HirLowerer {
            type_env: HashMap::new(),
            fn_sigs: HashMap::new(),
            counter: 0,
            diagnostics: Vec::new(),
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("__{}{}", prefix, n)
    }

    // ── Program entry point ───────────────────────────────────────────────────

    pub fn lower_program(
        &mut self,
        program: &ast::Program,
    ) -> Result<HirProgram, Vec<CompilerDiagnostic>> {
        self.diagnostics.clear();
        // Pass 1: collect function signatures for forward references
        for stmt in &program.statements {
            if let Statement::FunctionDeclaration(f) = stmt {
                let params: Vec<SzType> = f
                    .function
                    .parameters
                    .iter()
                    .map(|p| {
                        p.type_name
                            .as_deref()
                            .map(SzType::from_annotation)
                            .unwrap_or(SzType::Unknown)
                    })
                    .collect();
                let ret = f
                    .function
                    .return_type
                    .as_deref()
                    .map(SzType::from_annotation)
                    .unwrap_or(SzType::Void);
                self.fn_sigs.insert(f.name.clone(), (params, ret));
            }
        }

        let mut functions = Vec::new();
        let mut top_stmts: Vec<HirStmt> = Vec::new();

        for stmt in &program.statements {
            match stmt {
                Statement::FunctionDeclaration(f) => {
                    functions.push(self.lower_function(f));
                }
                Statement::ClassDeclaration(_) => {
                    self.unsupported_stmt("class declarations");
                }
                Statement::InterfaceDeclaration(_) => {
                    self.unsupported_stmt("interface declarations");
                }
                Statement::EnumDeclaration(_) => {
                    self.unsupported_stmt("enum declarations");
                }
                _ => {
                    top_stmts.extend(self.lower_stmt(stmt));
                }
            }
        }

        // Wrap top-level executable code in an implicit entry-point function
        if !top_stmts.is_empty() {
            functions.push(HirFunction {
                name: "__sz_main".to_string(),
                params: vec![],
                ret_type: SzType::Void,
                body: top_stmts,
            });
        }

        let hir = HirProgram { functions };
        if self.diagnostics.is_empty() {
            Ok(hir)
        } else {
            Err(std::mem::take(&mut self.diagnostics))
        }
    }

    fn unsupported_stmt(&mut self, construct: &str) {
        self.diagnostics.push(CompilerDiagnostic {
            code: "SZ7001",
            kind: "UnsupportedStatement",
            message: format!("the experimental compiler does not support {construct}"),
        });
    }

    fn unsupported_expr(&mut self, construct: &str) -> HirExpr {
        self.diagnostics.push(CompilerDiagnostic {
            code: "SZ7002",
            kind: "UnsupportedExpression",
            message: format!("the experimental compiler does not support {construct}"),
        });
        // Placeholder stays internal: lower_program returns Err and never exposes this HIR.
        HirExpr::Null
    }

    // ── Function ──────────────────────────────────────────────────────────────

    fn lower_function(&mut self, f: &ast::FunctionDeclaration) -> HirFunction {
        let params: Vec<HirParam> = f
            .function
            .parameters
            .iter()
            .map(|p| {
                let ty = p
                    .type_name
                    .as_deref()
                    .map(SzType::from_annotation)
                    .unwrap_or(SzType::Unknown);
                self.type_env.insert(p.name.clone(), ty.clone());
                HirParam {
                    name: p.name.clone(),
                    ty,
                }
            })
            .collect();

        let ret_type = f
            .function
            .return_type
            .as_deref()
            .map(SzType::from_annotation)
            .unwrap_or(SzType::Void);

        let body: Vec<HirStmt> = f
            .function
            .body
            .statements
            .iter()
            .flat_map(|s| self.lower_stmt(s))
            .collect();

        HirFunction {
            name: f.name.clone(),
            params,
            ret_type,
            body,
        }
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn lower_stmt(&mut self, stmt: &Statement) -> Vec<HirStmt> {
        match stmt {
            Statement::Let(l) => {
                let value = self.lower_expr(&l.value);
                let ty = value.ty();
                self.type_env.insert(l.name.clone(), ty.clone());
                vec![HirStmt::Let {
                    name: l.name.clone(),
                    ty,
                    value,
                    is_const: l.is_const,
                }]
            }

            Statement::Assign(a) => {
                let value = self.lower_expr(&a.value);
                vec![HirStmt::Assign(HirLValue::Var(a.name.clone()), value)]
            }

            Statement::Block(b) => {
                let stmts = b
                    .statements
                    .iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect();
                vec![HirStmt::Block(stmts)]
            }

            Statement::Unsafe(_) => {
                self.unsupported_stmt("unsafe blocks");
                vec![]
            }

            Statement::Return(r) => {
                vec![HirStmt::Return(Some(self.lower_expr(&r.return_value)))]
            }

            Statement::Expression(e) => {
                // If-expression used as statement → lower to HirStmt::If
                if let Expression::If(if_expr) = e {
                    return self.lower_if_stmt(if_expr);
                }
                vec![HirStmt::ExprStmt(self.lower_expr(e))]
            }

            Statement::While(w) => {
                let cond = self.lower_expr(&w.condition);
                let body = w
                    .body
                    .statements
                    .iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect();
                vec![HirStmt::While { cond, body }]
            }

            // DoWhile → { body; while (cond) { body } }
            Statement::DoWhile(w) => {
                let cond = self.lower_expr(&w.condition);
                let body: Vec<HirStmt> = w
                    .body
                    .statements
                    .iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect();
                let while_stmt = HirStmt::While {
                    cond,
                    body: body.clone(),
                };
                vec![HirStmt::Block(vec![HirStmt::Block(body), while_stmt])]
            }

            Statement::For(f) => {
                let init_val = self.lower_expr(&f.init.value);
                let init_ty = init_val.ty();
                self.type_env.insert(f.init.name.clone(), init_ty.clone());
                let init = HirStmt::Let {
                    name: f.init.name.clone(),
                    ty: init_ty,
                    value: init_val,
                    is_const: false,
                };
                let cond = self.lower_expr(&f.condition);
                let upd_val = self.lower_expr(&f.update.value);
                let update = HirStmt::Assign(HirLValue::Var(f.update.name.clone()), upd_val);
                let body = f
                    .body
                    .statements
                    .iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect();
                vec![HirStmt::For {
                    init: Box::new(init),
                    cond,
                    update: Box::new(update),
                    body,
                }]
            }

            Statement::ForEach(fe) => {
                self.unsupported_stmt("foreach loops");
                // Continue walking the iterable and body to report nested errors too.
                self.lower_expr(&fe.iterable);
                for stmt in &fe.body.statements {
                    self.lower_stmt(stmt);
                }
                vec![]
            }

            Statement::LetDestructureArray(_) => {
                self.unsupported_stmt("array destructuring");
                vec![]
            }
            Statement::LetDestructureDict(_) => {
                self.unsupported_stmt("dictionary destructuring");
                vec![]
            }
            Statement::Yield(_) => {
                self.unsupported_stmt("yield");
                vec![]
            }

            Statement::Out(o) => vec![HirStmt::Out(self.lower_expr(&o.value))],

            Statement::IndexAssign(ia) => {
                self.unsupported_stmt("index assignment");
                self.lower_expr(&ia.target);
                self.lower_expr(&ia.index);
                self.lower_expr(&ia.value);
                vec![]
            }

            Statement::FieldAssign(fa) => {
                self.unsupported_stmt("field assignment");
                self.lower_expr(&fa.value);
                vec![]
            }

            // a.b.c = v — el HIR sólo tiene un lvalue de UN salto
            // (`HirLValue::Field` sobre una variable), así que este caso todavía
            // no baja. Igual que Throw y DerefAssign, queda como no-op: este
            // backend está detrás de la feature `llvm` y sin usar.
            Statement::NestedFieldAssign(_) => {
                self.unsupported_stmt("nested field assignment");
                vec![]
            }

            Statement::Break => vec![HirStmt::Break],
            Statement::BreakLabel(_) => {
                self.unsupported_stmt("labeled break");
                vec![]
            }
            Statement::Continue => vec![HirStmt::Continue],
            Statement::ContinueLabel(_) => {
                self.unsupported_stmt("labeled continue");
                vec![]
            }

            // Switch → if/else chain
            Statement::Switch(sw) => {
                let val_expr = self.lower_expr(&sw.value);
                let val_ty = val_expr.ty();
                let tmp = self.fresh("sw");
                self.type_env.insert(tmp.clone(), val_ty.clone());

                let let_tmp = HirStmt::Let {
                    name: tmp.clone(),
                    ty: val_ty.clone(),
                    value: val_expr,
                    is_const: true,
                };

                let default_body: Vec<HirStmt> = sw
                    .default
                    .as_ref()
                    .map(|d| {
                        d.statements
                            .iter()
                            .flat_map(|s| self.lower_stmt(s))
                            .collect()
                    })
                    .unwrap_or_default();

                let chain = sw.cases.iter().rev().fold(default_body, |else_body, case| {
                    let cond = case.values.iter().enumerate().fold(
                        HirExpr::LitBool(false),
                        |acc, (i, v)| {
                            let eq = HirExpr::BinOp {
                                op: HirBinOp::Eq,
                                left: Box::new(HirExpr::Var(tmp.clone(), val_ty.clone())),
                                right: Box::new(self.lower_expr(v)),
                                ty: SzType::Bool,
                            };
                            if i == 0 {
                                eq
                            } else {
                                HirExpr::BinOp {
                                    op: HirBinOp::Or,
                                    left: Box::new(acc),
                                    right: Box::new(eq),
                                    ty: SzType::Bool,
                                }
                            }
                        },
                    );
                    let then_body = case
                        .body
                        .statements
                        .iter()
                        .flat_map(|s| self.lower_stmt(s))
                        .collect();
                    vec![HirStmt::If {
                        cond,
                        then_body,
                        else_body,
                    }]
                });

                let mut result = vec![let_tmp];
                result.extend(chain);
                vec![HirStmt::Block(result)]
            }

            // Try/Catch: phase 1 — lower only the guarded body; exception support comes later
            Statement::Try(_) => {
                self.unsupported_stmt("try/catch/finally");
                vec![]
            }

            // Throw: phase 1 — no-op; full exception support comes later
            Statement::Throw(_) => {
                self.unsupported_stmt("throw");
                vec![]
            }

            // Pointer write — stub (native pointer support in Phase 1.5+)
            Statement::DerefAssign { .. } => {
                self.unsupported_stmt("pointer assignment");
                vec![]
            }

            // Native function declaration — no HIR; dispatch is at runtime
            Statement::NativeDeclaration(_) => {
                self.unsupported_stmt("native declarations");
                vec![]
            }

            // Import/Export/Permissions — resolved at eval time, not compile time
            Statement::Import(_) => {
                self.unsupported_stmt("imports");
                vec![]
            }
            Statement::UsePermissions(_) => {
                self.unsupported_stmt("permission declarations");
                vec![]
            }
            Statement::Export(inner) => self.lower_stmt(inner),

            // Already handled at program level
            Statement::FunctionDeclaration(_) => {
                self.unsupported_stmt("nested function declarations");
                vec![]
            }
            Statement::ClassDeclaration(_) => {
                self.unsupported_stmt("nested class declarations");
                vec![]
            }
            Statement::InterfaceDeclaration(_) => {
                self.unsupported_stmt("nested interface declarations");
                vec![]
            }
            Statement::EnumDeclaration(_) => {
                self.unsupported_stmt("nested enum declarations");
                vec![]
            }
        }
    }

    /// Lower an if-expression when used in statement position.
    fn lower_if_stmt(&mut self, if_expr: &ast::IfExpression) -> Vec<HirStmt> {
        let cond = self.lower_expr(&if_expr.condition);
        let then_body = if_expr
            .consequence
            .statements
            .iter()
            .flat_map(|s| self.lower_stmt(s))
            .collect();
        let else_body = if_expr
            .alternative
            .as_ref()
            .map(|alt| {
                alt.statements
                    .iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect()
            })
            .unwrap_or_default();
        vec![HirStmt::If {
            cond,
            then_body,
            else_body,
        }]
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn lower_expr(&mut self, expr: &Expression) -> HirExpr {
        match expr {
            Expression::Integer(i) => HirExpr::LitInt(*i),
            Expression::Decimal(d) => HirExpr::LitDecimal(*d),
            // The LLVM backend has no exact-decimal type; `dec` lowers to f64
            // (lossy). Exact arithmetic is only guaranteed on the interpreter.
            Expression::Dec(_) => self.unsupported_expr("exact decimal literals"),
            Expression::Boolean(b) => HirExpr::LitBool(*b),
            Expression::String(s) => HirExpr::LitStr(s.clone()),
            Expression::Null => HirExpr::Null,

            Expression::Identifier(name) => {
                let ty = self.type_env.get(name).cloned().unwrap_or(SzType::Unknown);
                HirExpr::Var(name.clone(), ty)
            }

            Expression::Prefix(op, operand) => {
                let operand = self.lower_expr(operand);
                let ty = operand.ty();
                let hir_op = match op.as_str() {
                    "!" => HirUnaryOp::Not,
                    _ => HirUnaryOp::Neg,
                };
                HirExpr::UnaryOp {
                    op: hir_op,
                    operand: Box::new(operand),
                    ty,
                }
            }

            Expression::Infix(infix) => {
                // Null coalescing: a ?? b → if (a != null) a else b
                if infix.operator == "??" {
                    let left = self.lower_expr(&infix.left);
                    let right = self.lower_expr(&infix.right);
                    let ty = left.ty();
                    let cond = HirExpr::BinOp {
                        op: HirBinOp::Ne,
                        left: Box::new(left.clone()),
                        right: Box::new(HirExpr::Null),
                        ty: SzType::Bool,
                    };
                    return HirExpr::If {
                        cond: Box::new(cond),
                        then_expr: Box::new(left),
                        else_expr: Box::new(right),
                        ty,
                    };
                }

                let left = self.lower_expr(&infix.left);
                let right = self.lower_expr(&infix.right);
                let op = self.map_binop(&infix.operator);
                let ty = self.binop_result_ty(&op, &left.ty(), &right.ty());
                HirExpr::BinOp {
                    op,
                    left: Box::new(left),
                    right: Box::new(right),
                    ty,
                }
            }

            Expression::Call(call) => {
                let name = match call.function.as_ref() {
                    Expression::Identifier(n) => n.clone(),
                    _ => {
                        self.unsupported_expr("calls through computed expressions");
                        "__invalid_call".to_string()
                    }
                };
                let args: Vec<HirExpr> =
                    call.arguments.iter().map(|a| self.lower_expr(a)).collect();
                let ty = self
                    .fn_sigs
                    .get(&name)
                    .map(|(_, ret)| ret.clone())
                    .unwrap_or(SzType::Unknown);
                HirExpr::Call { name, args, ty }
            }

            Expression::DotCall(dc) => {
                self.unsupported_expr(if dc.has_parens {
                    "method calls"
                } else {
                    "field access"
                });
                self.lower_expr(&dc.object);
                for arg in &dc.arguments {
                    self.lower_expr(arg);
                }
                HirExpr::Null
            }

            // Ternary: cond ? a : b → HirExpr::If
            Expression::Ternary(t) => {
                let cond = self.lower_expr(&t.condition);
                let then_expr = self.lower_expr(&t.then_expr);
                let else_expr = self.lower_expr(&t.else_expr);
                let ty = then_expr.ty();
                HirExpr::If {
                    cond: Box::new(cond),
                    then_expr: Box::new(then_expr),
                    else_expr: Box::new(else_expr),
                    ty,
                }
            }

            Expression::If(_) => self.unsupported_expr("if expressions"),

            Expression::Index(idx) => {
                self.unsupported_expr("index access");
                self.lower_expr(&idx.left);
                self.lower_expr(&idx.index);
                HirExpr::Null
            }

            Expression::New(n) => {
                self.unsupported_expr("object construction");
                match &n.args {
                    NewArgs::Positional(args) => {
                        for arg in args {
                            self.lower_expr(arg);
                        }
                    }
                    NewArgs::Fields(fields) => {
                        for (_, value) in fields {
                            self.lower_expr(value);
                        }
                    }
                }
                HirExpr::Null
            }

            Expression::ArrayLiteral(arr) => {
                self.unsupported_expr("array literals");
                for element in &arr.elements {
                    self.lower_expr(element);
                }
                HirExpr::Null
            }

            // "Hello {name}!" → "Hello " + name.toString()
            Expression::InterpolatedString(parts) => {
                self.unsupported_expr("interpolated strings");
                for part in parts {
                    if let StringPart::Expr(expr) = part {
                        self.lower_expr(expr);
                    }
                }
                HirExpr::Null
            }

            // sizeof → constant integer at HIR level
            Expression::SizeOf(target) => {
                use crate::ast::SizeOfTarget;
                let size: i64 = match target {
                    SizeOfTarget::Type(name) => match name.as_str() {
                        "int" | "decimal" | "string" | "any" => 8,
                        "bool" => 1,
                        "null" | "void" => 0,
                        _ => 8,
                    },
                    SizeOfTarget::Expr(_) => 8, // conservative: pointer-sized at HIR
                };
                HirExpr::LitInt(size)
            }

            // Pointer expressions — stub as Null until native pointer support lands
            Expression::AddressOf(_) => self.unsupported_expr("address-of expressions"),
            Expression::Deref(_) => self.unsupported_expr("pointer dereference expressions"),

            // Phase 1: lambdas, dicts, spread, object-patch are unsupported
            Expression::FunctionLiteral(_) => self.unsupported_expr("function literals"),
            Expression::Lambda(_) => self.unsupported_expr("lambdas"),
            Expression::DictLiteral(_) => self.unsupported_expr("dictionary literals"),
            Expression::EntryLiteral(_, _) => self.unsupported_expr("dictionary entries"),
            Expression::ObjectPatch(_) => self.unsupported_expr("object patches"),
            Expression::Spread(_) => self.unsupported_expr("spread expressions"),
            Expression::Match(_) => self.unsupported_expr("match expressions"),
            Expression::UnsafeBlock(_) => self.unsupported_expr("unsafe expressions"),
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn map_binop(&self, op: &str) -> HirBinOp {
        match op {
            "+" => HirBinOp::Add,
            "-" => HirBinOp::Sub,
            "*" => HirBinOp::Mul,
            "/" => HirBinOp::Div,
            "%" => HirBinOp::Mod,
            "**" => HirBinOp::Pow,
            "==" => HirBinOp::Eq,
            "!=" => HirBinOp::Ne,
            "<" => HirBinOp::Lt,
            "<=" => HirBinOp::Le,
            ">" => HirBinOp::Gt,
            ">=" => HirBinOp::Ge,
            "&&" => HirBinOp::And,
            "||" => HirBinOp::Or,
            "&" => HirBinOp::BitAnd,
            "|" => HirBinOp::BitOr,
            "^" => HirBinOp::BitXor,
            "<<" => HirBinOp::Shl,
            ">>" => HirBinOp::Shr,
            _ => HirBinOp::Add,
        }
    }

    fn binop_result_ty(&self, op: &HirBinOp, left: &SzType, right: &SzType) -> SzType {
        match op {
            HirBinOp::Eq
            | HirBinOp::Ne
            | HirBinOp::Lt
            | HirBinOp::Le
            | HirBinOp::Gt
            | HirBinOp::Ge
            | HirBinOp::And
            | HirBinOp::Or => SzType::Bool,
            _ => match (left, right) {
                (SzType::Str, _) | (_, SzType::Str) => SzType::Str,
                (SzType::Decimal, _) | (_, SzType::Decimal) => SzType::Decimal,
                _ => left.clone(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::lexer::Lexer;
    use crate::parser::Parser;

    // ── AST builder helpers ───────────────────────────────────────────────────

    fn program(stmts: Vec<ast::Statement>) -> ast::Program {
        ast::Program { statements: stmts }
    }

    fn lower_source(source: &str) -> Result<HirProgram, Vec<CompilerDiagnostic>> {
        let mut parser = Parser::new(Lexer::new(source.to_string()));
        let program = parser.parse_program();
        assert!(
            !parser.has_errors(),
            "test source must parse: {:?}",
            parser.take_errors()
        );
        HirLowerer::new().lower_program(&program)
    }

    fn block(stmts: Vec<ast::Statement>) -> ast::BlockStatement {
        ast::BlockStatement { statements: stmts }
    }

    fn let_int(name: &str, val: i64) -> ast::Statement {
        ast::Statement::Let(ast::LetStatement {
            name: name.to_string(),
            value: ast::Expression::Integer(val),
            is_const: false,
        })
    }

    fn let_bool(name: &str, val: bool) -> ast::Statement {
        ast::Statement::Let(ast::LetStatement {
            name: name.to_string(),
            value: ast::Expression::Boolean(val),
            is_const: false,
        })
    }

    fn infix(l: ast::Expression, op: &str, r: ast::Expression) -> ast::Expression {
        ast::Expression::Infix(ast::InfixExpression {
            left: Box::new(l),
            operator: op.to_string(),
            right: Box::new(r),
            line: 0,
            column: 0,
        })
    }

    fn ident(name: &str) -> ast::Expression {
        ast::Expression::Identifier(name.to_string())
    }

    fn out(expr: ast::Expression) -> ast::Statement {
        ast::Statement::Out(ast::OutStatement { value: expr })
    }

    fn fn_decl(
        name: &str,
        params: Vec<(&str, &str)>,
        ret: &str,
        body: Vec<ast::Statement>,
    ) -> ast::Statement {
        ast::Statement::FunctionDeclaration(ast::FunctionDeclaration {
            name: name.to_string(),
            function: ast::FunctionLiteral {
                return_type: Some(ret.to_string()),
                parameters: params
                    .iter()
                    .map(|(n, t)| ast::Parameter {
                        name: n.to_string(),
                        type_name: Some(t.to_string()),
                        is_rest: false,
                        default_value: None,
                    })
                    .collect(),
                body: block(body),
                is_generator: false,
            },
        })
    }

    fn main_fn(hir: &crate::compiler::hir::HirProgram) -> &crate::compiler::hir::HirFunction {
        hir.functions
            .iter()
            .find(|f| f.name == "__sz_main")
            .expect("no __sz_main")
    }

    // ── Let / Assign ─────────────────────────────────────────────────────────

    #[test]
    fn let_integer_lowers_to_hir_let() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![let_int("x", 99)]))
            .unwrap();
        let m = main_fn(&hir);
        assert_eq!(m.body.len(), 1);
        match &m.body[0] {
            HirStmt::Let {
                name, ty, value, ..
            } => {
                assert_eq!(name, "x");
                assert_eq!(*ty, SzType::Int);
                assert!(matches!(value, HirExpr::LitInt(99)));
            }
            s => panic!("expected Let, got {:?}", s),
        }
    }

    #[test]
    fn let_bool_infers_bool_type() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![let_bool("flag", false)]))
            .unwrap();
        let m = main_fn(&hir);
        match &m.body[0] {
            HirStmt::Let { ty, value, .. } => {
                assert_eq!(*ty, SzType::Bool);
                assert!(matches!(value, HirExpr::LitBool(false)));
            }
            s => panic!("{:?}", s),
        }
    }

    #[test]
    fn multiple_top_level_stmts_all_go_to_sz_main() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![
                let_int("a", 1),
                let_int("b", 2),
                let_int("c", 3),
            ]))
            .unwrap();
        let m = main_fn(&hir);
        assert_eq!(m.body.len(), 3);
    }

    // ── Arithmetic / BinOp ───────────────────────────────────────────────────

    #[test]
    fn addition_becomes_binop_add() {
        let expr = infix(
            ast::Expression::Integer(3),
            "+",
            ast::Expression::Integer(4),
        );
        let hir = HirLowerer::new()
            .lower_program(&program(vec![ast::Statement::Let(ast::LetStatement {
                name: "r".into(),
                value: expr,
                is_const: false,
            })]))
            .unwrap();
        match &main_fn(&hir).body[0] {
            HirStmt::Let {
                value: HirExpr::BinOp { op, ty, .. },
                ..
            } => {
                assert_eq!(*op, HirBinOp::Add);
                assert_eq!(*ty, SzType::Int);
            }
            s => panic!("{:?}", s),
        }
    }

    #[test]
    fn comparison_produces_bool_type() {
        let expr = infix(ident("x"), "<", ast::Expression::Integer(10));
        let hir = HirLowerer::new()
            .lower_program(&program(vec![ast::Statement::Let(ast::LetStatement {
                name: "c".into(),
                value: expr,
                is_const: false,
            })]))
            .unwrap();
        match &main_fn(&hir).body[0] {
            HirStmt::Let {
                ty,
                value: HirExpr::BinOp { op, .. },
                ..
            } => {
                assert_eq!(*ty, SzType::Bool);
                assert_eq!(*op, HirBinOp::Lt);
            }
            s => panic!("{:?}", s),
        }
    }

    // ── Control flow ─────────────────────────────────────────────────────────

    #[test]
    fn out_statement_lowers_correctly() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![out(ast::Expression::Integer(42))]))
            .unwrap();
        assert!(matches!(
            &main_fn(&hir).body[0],
            HirStmt::Out(HirExpr::LitInt(42))
        ));
    }

    #[test]
    fn while_loop_lowers_to_hir_while() {
        let w = ast::Statement::While(ast::WhileStatement {
            condition: ast::Expression::Boolean(true),
            body: block(vec![ast::Statement::Break]),
            label: None,
        });
        let hir = HirLowerer::new().lower_program(&program(vec![w])).unwrap();
        let stmt = &main_fn(&hir).body[0];
        assert!(matches!(stmt, HirStmt::While { .. }));
        if let HirStmt::While { cond, body } = stmt {
            assert!(matches!(cond, HirExpr::LitBool(true)));
            assert!(matches!(body[0], HirStmt::Break));
        }
    }

    #[test]
    fn break_and_continue_lower_directly() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![
                ast::Statement::Break,
                ast::Statement::Continue,
            ]))
            .unwrap();
        let m = main_fn(&hir);
        assert!(matches!(m.body[0], HirStmt::Break));
        assert!(matches!(m.body[1], HirStmt::Continue));
    }

    #[test]
    fn if_statement_with_else_lowers_correctly() {
        let if_stmt = ast::Statement::Expression(ast::Expression::If(ast::IfExpression {
            condition: Box::new(ast::Expression::Boolean(true)),
            consequence: block(vec![out(ast::Expression::Integer(1))]),
            alternative: Some(block(vec![out(ast::Expression::Integer(2))])),
        }));
        let hir = HirLowerer::new()
            .lower_program(&program(vec![if_stmt]))
            .unwrap();
        let m = main_fn(&hir);
        match &m.body[0] {
            HirStmt::If {
                cond,
                then_body,
                else_body,
            } => {
                assert!(matches!(cond, HirExpr::LitBool(true)));
                assert_eq!(then_body.len(), 1);
                assert_eq!(else_body.len(), 1);
            }
            s => panic!("{:?}", s),
        }
    }

    #[test]
    fn do_while_desugars_to_body_plus_while() {
        let dw = ast::Statement::DoWhile(ast::WhileStatement {
            condition: ast::Expression::Boolean(false),
            body: block(vec![out(ast::Expression::Integer(0))]),
            label: None,
        });
        let hir = HirLowerer::new().lower_program(&program(vec![dw])).unwrap();
        // DoWhile → Block([Block(body), While{...}])
        match &main_fn(&hir).body[0] {
            HirStmt::Block(outer) => {
                assert!(outer.iter().any(|s| matches!(s, HirStmt::While { .. })));
            }
            s => panic!("expected Block from DoWhile, got {:?}", s),
        }
    }

    #[test]
    fn ternary_desugars_to_hir_if_expr() {
        let ternary = ast::Expression::Ternary(ast::TernaryExpression {
            condition: Box::new(ast::Expression::Boolean(true)),
            then_expr: Box::new(ast::Expression::Integer(1)),
            else_expr: Box::new(ast::Expression::Integer(0)),
        });
        let hir = HirLowerer::new()
            .lower_program(&program(vec![ast::Statement::Let(ast::LetStatement {
                name: "v".into(),
                value: ternary,
                is_const: false,
            })]))
            .unwrap();
        match &main_fn(&hir).body[0] {
            HirStmt::Let {
                value: HirExpr::If { .. },
                ..
            } => {}
            s => panic!("expected Let with HirExpr::If, got {:?}", s),
        }
    }

    #[test]
    fn null_coalescing_desugars_to_hir_if_expr() {
        let nc = infix(ident("maybe"), "??", ast::Expression::Integer(0));
        let hir = HirLowerer::new()
            .lower_program(&program(vec![ast::Statement::Let(ast::LetStatement {
                name: "v".into(),
                value: nc,
                is_const: false,
            })]))
            .unwrap();
        match &main_fn(&hir).body[0] {
            HirStmt::Let {
                value: HirExpr::If { .. },
                ..
            } => {}
            s => panic!("expected Let with HirExpr::If from ??, got {:?}", s),
        }
    }

    #[test]
    fn switch_desugars_to_if_else_chain() {
        let sw = ast::Statement::Switch(ast::SwitchStatement {
            value: ident("x"),
            cases: vec![
                ast::SwitchCase {
                    values: vec![ast::Expression::Integer(1)],
                    body: block(vec![out(ast::Expression::Integer(10))]),
                },
                ast::SwitchCase {
                    values: vec![ast::Expression::Integer(2)],
                    body: block(vec![out(ast::Expression::Integer(20))]),
                },
            ],
            default: Some(block(vec![out(ast::Expression::Integer(0))])),
        });
        let hir = HirLowerer::new().lower_program(&program(vec![sw])).unwrap();
        // Switch → Block([let_tmp, If{...}])
        match &main_fn(&hir).body[0] {
            HirStmt::Block(stmts) => {
                // first stmt is the temp let binding
                assert!(matches!(stmts[0], HirStmt::Let { .. }));
                // rest are if/else
                assert!(matches!(stmts[1], HirStmt::If { .. }));
            }
            s => panic!("expected Block from switch, got {:?}", s),
        }
    }

    // ── Function declarations ─────────────────────────────────────────────────

    #[test]
    fn function_params_and_return_type_resolved() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![fn_decl(
                "add",
                vec![("a", "int"), ("b", "int")],
                "int",
                vec![ast::Statement::Return(ast::ReturnStatement {
                    return_value: infix(ident("a"), "+", ident("b")),
                })],
            )]))
            .unwrap();
        let f = hir.functions.iter().find(|f| f.name == "add").unwrap();
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].ty, SzType::Int);
        assert_eq!(f.params[1].ty, SzType::Int);
        assert_eq!(f.ret_type, SzType::Int);
        assert!(matches!(f.body[0], HirStmt::Return(Some(_))));
    }

    #[test]
    fn function_void_return_is_void() {
        let hir = HirLowerer::new()
            .lower_program(&program(vec![fn_decl(
                "greet",
                vec![],
                "void",
                vec![out(ast::Expression::String("hi".to_string()))],
            )]))
            .unwrap();
        let f = hir.functions.iter().find(|f| f.name == "greet").unwrap();
        assert_eq!(f.ret_type, SzType::Void);
        assert_eq!(f.params.len(), 0);
    }

    #[test]
    fn foreach_is_rejected_until_backend_support_is_complete() {
        let fe = ast::Statement::ForEach(ast::ForEachStatement {
            var: ast::ForEachVar::Name("n".to_string()),
            iterable: ident("items"),
            body: block(vec![out(ident("n"))]),
            label: None,
        });
        let errors = HirLowerer::new()
            .lower_program(&program(vec![fe]))
            .unwrap_err();
        assert!(
            errors
                .iter()
                .any(|error| { error.code == "SZ7001" && error.message.contains("foreach loops") })
        );
    }

    // ── App: fibonacci function ───────────────────────────────────────────────

    #[test]
    fn fibonacci_function_structure() {
        // fn int fib(int n) { if (n <= 1) { return n; } return fib(n-1) + fib(n-2); }
        let body = vec![
            ast::Statement::Expression(ast::Expression::If(ast::IfExpression {
                condition: Box::new(infix(ident("n"), "<=", ast::Expression::Integer(1))),
                consequence: block(vec![ast::Statement::Return(ast::ReturnStatement {
                    return_value: ident("n"),
                })]),
                alternative: None,
            })),
            ast::Statement::Return(ast::ReturnStatement {
                return_value: infix(
                    ast::Expression::Call(ast::CallExpression {
                        function: Box::new(ident("fib")),
                        arguments: vec![infix(ident("n"), "-", ast::Expression::Integer(1))],
                        line: 0,
                        column: 0,
                    }),
                    "+",
                    ast::Expression::Call(ast::CallExpression {
                        function: Box::new(ident("fib")),
                        arguments: vec![infix(ident("n"), "-", ast::Expression::Integer(2))],
                        line: 0,
                        column: 0,
                    }),
                ),
            }),
        ];
        let hir = HirLowerer::new()
            .lower_program(&program(vec![fn_decl(
                "fib",
                vec![("n", "int")],
                "int",
                body,
            )]))
            .unwrap();
        let f = hir.functions.iter().find(|f| f.name == "fib").unwrap();
        assert_eq!(f.ret_type, SzType::Int);
        assert_eq!(f.params[0].name, "n");
        // body: If + Return
        assert_eq!(f.body.len(), 2);
        assert!(matches!(f.body[0], HirStmt::If { .. }));
        assert!(matches!(f.body[1], HirStmt::Return(Some(_))));
    }

    #[test]
    fn unsupported_expression_returns_sz7002_instead_of_null_hir() {
        let errors = lower_source("let double = x => x * 2;").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SZ7002");
        assert_eq!(errors[0].kind, "UnsupportedExpression");
        assert!(errors[0].message.contains("lambdas"));
    }

    #[test]
    fn unsupported_statement_returns_sz7001_instead_of_noop() {
        let errors = lower_source("throw \"boom\";").unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, "SZ7001");
        assert_eq!(errors[0].kind, "UnsupportedStatement");
        assert!(errors[0].message.contains("throw"));
    }

    #[test]
    fn lowering_reports_every_unsupported_construct_before_aborting() {
        let errors =
            lower_source("let precise = 0.21m; let f = x => x; throw precise;").unwrap_err();
        assert_eq!(errors.len(), 3);
        assert_eq!(errors.iter().filter(|e| e.code == "SZ7002").count(), 2);
        assert_eq!(errors.iter().filter(|e| e.code == "SZ7001").count(), 1);
    }
}
