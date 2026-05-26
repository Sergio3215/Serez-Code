/// AST → HIR lowering.
///
/// Walks the source AST and produces the High-level IR:
///   - Resolves types from annotations and inference
///   - Desugars complex constructs into simpler HIR forms
///   - Wraps top-level statements in an implicit `__sz_main` function
use std::collections::HashMap;
use crate::ast::{self, Expression, NewArgs, Statement, StringPart};
use crate::compiler::hir::*;
use crate::compiler::types::SzType;

pub struct HirLowerer {
    /// Variable name → inferred compile-time type
    type_env: HashMap<String, SzType>,
    /// Function name → (param types, return type)
    fn_sigs: HashMap<String, (Vec<SzType>, SzType)>,
    /// Counter for generating unique synthetic variable names
    counter: usize,
}

impl HirLowerer {
    pub fn new() -> Self {
        HirLowerer {
            type_env: HashMap::new(),
            fn_sigs: HashMap::new(),
            counter: 0,
        }
    }

    fn fresh(&mut self, prefix: &str) -> String {
        let n = self.counter;
        self.counter += 1;
        format!("__{}{}", prefix, n)
    }

    // ── Program entry point ───────────────────────────────────────────────────

    pub fn lower_program(&mut self, program: &ast::Program) -> HirProgram {
        // Pass 1: collect function signatures for forward references
        for stmt in &program.statements {
            if let Statement::FunctionDeclaration(f) = stmt {
                let params: Vec<SzType> = f.function.parameters.iter()
                    .map(|p| p.type_name.as_deref().map(SzType::from_annotation).unwrap_or(SzType::Unknown))
                    .collect();
                let ret = f.function.return_type.as_deref()
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
                // Classes, interfaces and enums are type-level — lowered in a later phase
                Statement::ClassDeclaration(_)
                | Statement::InterfaceDeclaration(_)
                | Statement::EnumDeclaration(_) => {}
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

        HirProgram { functions }
    }

    // ── Function ──────────────────────────────────────────────────────────────

    fn lower_function(&mut self, f: &ast::FunctionDeclaration) -> HirFunction {
        let params: Vec<HirParam> = f.function.parameters.iter().map(|p| {
            let ty = p.type_name.as_deref().map(SzType::from_annotation).unwrap_or(SzType::Unknown);
            self.type_env.insert(p.name.clone(), ty.clone());
            HirParam { name: p.name.clone(), ty }
        }).collect();

        let ret_type = f.function.return_type.as_deref()
            .map(SzType::from_annotation)
            .unwrap_or(SzType::Void);

        let body: Vec<HirStmt> = f.function.body.statements.iter()
            .flat_map(|s| self.lower_stmt(s))
            .collect();

        HirFunction { name: f.name.clone(), params, ret_type, body }
    }

    // ── Statements ────────────────────────────────────────────────────────────

    fn lower_stmt(&mut self, stmt: &Statement) -> Vec<HirStmt> {
        match stmt {
            Statement::Let(l) => {
                let value = self.lower_expr(&l.value);
                let ty = value.ty();
                self.type_env.insert(l.name.clone(), ty.clone());
                vec![HirStmt::Let { name: l.name.clone(), ty, value, is_const: l.is_const }]
            }

            Statement::Assign(a) => {
                let value = self.lower_expr(&a.value);
                vec![HirStmt::Assign(HirLValue::Var(a.name.clone()), value)]
            }

            Statement::Block(b) | Statement::Unsafe(b) => {
                let stmts = b.statements.iter().flat_map(|s| self.lower_stmt(s)).collect();
                vec![HirStmt::Block(stmts)]
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
                let body = w.body.statements.iter().flat_map(|s| self.lower_stmt(s)).collect();
                vec![HirStmt::While { cond, body }]
            }

            // DoWhile → { body; while (cond) { body } }
            Statement::DoWhile(w) => {
                let cond = self.lower_expr(&w.condition);
                let body: Vec<HirStmt> = w.body.statements.iter()
                    .flat_map(|s| self.lower_stmt(s))
                    .collect();
                let while_stmt = HirStmt::While { cond, body: body.clone() };
                vec![HirStmt::Block(vec![HirStmt::Block(body), while_stmt])]
            }

            Statement::For(f) => {
                let init_val = self.lower_expr(&f.init.value);
                let init_ty = init_val.ty();
                self.type_env.insert(f.init.name.clone(), init_ty.clone());
                let init = HirStmt::Let {
                    name: f.init.name.clone(), ty: init_ty, value: init_val, is_const: false,
                };
                let cond = self.lower_expr(&f.condition);
                let upd_val = self.lower_expr(&f.update.value);
                let update = HirStmt::Assign(HirLValue::Var(f.update.name.clone()), upd_val);
                let body = f.body.statements.iter().flat_map(|s| self.lower_stmt(s)).collect();
                vec![HirStmt::For {
                    init: Box::new(init), cond, update: Box::new(update), body,
                }]
            }

            // ForEach(x in arr) → { let __iter = arr; for (let __i = 0; __i < __iter.length; __i++) { let x = __iter[__i]; body } }
            Statement::ForEach(fe) => {
                let iter_name = self.fresh("iter");
                let idx_name  = self.fresh("idx");
                let len_name  = self.fresh("len");

                let iter_expr = self.lower_expr(&fe.iterable);
                let iter_ty = iter_expr.ty();
                let elem_ty = match &iter_ty {
                    SzType::Array(t) => *t.clone(),
                    _ => SzType::Unknown,
                };

                let var_name = match &fe.var {
                    ast::ForEachVar::Name(n) => n.clone(),
                    ast::ForEachVar::Array(_, _) => self.fresh("item"),
                };

                self.type_env.insert(iter_name.clone(), iter_ty.clone());
                self.type_env.insert(idx_name.clone(), SzType::Int);
                self.type_env.insert(len_name.clone(), SzType::Int);
                self.type_env.insert(var_name.clone(), elem_ty.clone());

                let let_iter = HirStmt::Let {
                    name: iter_name.clone(), ty: iter_ty.clone(),
                    value: iter_expr, is_const: true,
                };
                let let_len = HirStmt::Let {
                    name: len_name.clone(), ty: SzType::Int,
                    value: HirExpr::MethodCall {
                        object: Box::new(HirExpr::Var(iter_name.clone(), iter_ty.clone())),
                        method: "length".to_string(), args: vec![], ty: SzType::Int,
                    },
                    is_const: true,
                };

                let init = HirStmt::Let {
                    name: idx_name.clone(), ty: SzType::Int,
                    value: HirExpr::LitInt(0), is_const: false,
                };
                let cond = HirExpr::BinOp {
                    op: HirBinOp::Lt,
                    left:  Box::new(HirExpr::Var(idx_name.clone(), SzType::Int)),
                    right: Box::new(HirExpr::Var(len_name.clone(), SzType::Int)),
                    ty: SzType::Bool,
                };
                let update = HirStmt::Assign(
                    HirLValue::Var(idx_name.clone()),
                    HirExpr::BinOp {
                        op: HirBinOp::Add,
                        left:  Box::new(HirExpr::Var(idx_name.clone(), SzType::Int)),
                        right: Box::new(HirExpr::LitInt(1)),
                        ty: SzType::Int,
                    },
                );

                let let_elem = HirStmt::Let {
                    name: var_name.clone(), ty: elem_ty.clone(),
                    value: HirExpr::Index {
                        array: Box::new(HirExpr::Var(iter_name, iter_ty)),
                        index: Box::new(HirExpr::Var(idx_name, SzType::Int)),
                        ty: elem_ty,
                    },
                    is_const: true,
                };

                let mut body = vec![let_elem];
                body.extend(fe.body.statements.iter().flat_map(|s| self.lower_stmt(s)));

                vec![HirStmt::Block(vec![
                    let_iter, let_len,
                    HirStmt::For { init: Box::new(init), cond, update: Box::new(update), body },
                ])]
            }

            Statement::LetDestructureArray(_) | Statement::LetDestructureDict(_) => vec![],
            Statement::Yield(_) => vec![],

            Statement::Out(o) => vec![HirStmt::Out(self.lower_expr(&o.value))],

            Statement::IndexAssign(ia) => {
                let array = self.lower_expr(&ia.target);
                let index = self.lower_expr(&ia.index);
                let value = self.lower_expr(&ia.value);
                vec![HirStmt::Assign(
                    HirLValue::Index { array: Box::new(array), index: Box::new(index) },
                    value,
                )]
            }

            Statement::FieldAssign(fa) => {
                let ty = self.type_env.get(&fa.object).cloned().unwrap_or(SzType::Unknown);
                let object = HirExpr::Var(fa.object.clone(), ty);
                let value = self.lower_expr(&fa.value);
                vec![HirStmt::Assign(
                    HirLValue::Field { object: Box::new(object), field: fa.field.clone() },
                    value,
                )]
            }

            Statement::Break | Statement::BreakLabel(_)       => vec![HirStmt::Break],
            Statement::Continue | Statement::ContinueLabel(_) => vec![HirStmt::Continue],

            // Switch → if/else chain
            Statement::Switch(sw) => {
                let val_expr = self.lower_expr(&sw.value);
                let val_ty = val_expr.ty();
                let tmp = self.fresh("sw");
                self.type_env.insert(tmp.clone(), val_ty.clone());

                let let_tmp = HirStmt::Let {
                    name: tmp.clone(), ty: val_ty.clone(),
                    value: val_expr, is_const: true,
                };

                let default_body: Vec<HirStmt> = sw.default.as_ref().map(|d| {
                    d.statements.iter().flat_map(|s| self.lower_stmt(s)).collect()
                }).unwrap_or_default();

                let chain = sw.cases.iter().rev().fold(default_body, |else_body, case| {
                    let cond = case.values.iter().enumerate().fold(
                        HirExpr::LitBool(false),
                        |acc, (i, v)| {
                            let eq = HirExpr::BinOp {
                                op: HirBinOp::Eq,
                                left:  Box::new(HirExpr::Var(tmp.clone(), val_ty.clone())),
                                right: Box::new(self.lower_expr(v)),
                                ty: SzType::Bool,
                            };
                            if i == 0 { eq } else {
                                HirExpr::BinOp {
                                    op: HirBinOp::Or,
                                    left: Box::new(acc), right: Box::new(eq),
                                    ty: SzType::Bool,
                                }
                            }
                        },
                    );
                    let then_body = case.body.statements.iter()
                        .flat_map(|s| self.lower_stmt(s))
                        .collect();
                    vec![HirStmt::If { cond, then_body, else_body }]
                });

                let mut result = vec![let_tmp];
                result.extend(chain);
                vec![HirStmt::Block(result)]
            }

            // Try/Catch: phase 1 — lower only the guarded body; exception support comes later
            Statement::Try(t) => {
                t.body.statements.iter().flat_map(|s| self.lower_stmt(s)).collect()
            }

            // Throw: phase 1 — no-op; full exception support comes later
            Statement::Throw(_) => vec![],

            // Pointer write — stub (native pointer support in Phase 1.5+)
            Statement::DerefAssign { .. } => vec![],

            // Native function declaration — no HIR; dispatch is at runtime
            Statement::NativeDeclaration(_) => vec![],

            // Import/Export — resolved at eval time, not compile time
            Statement::Import(_) => vec![],
            Statement::Export(inner) => self.lower_stmt(inner),

            // Already handled at program level
            Statement::FunctionDeclaration(_)
            | Statement::ClassDeclaration(_)
            | Statement::InterfaceDeclaration(_)
            | Statement::EnumDeclaration(_) => vec![],
        }
    }

    /// Lower an if-expression when used in statement position.
    fn lower_if_stmt(&mut self, if_expr: &ast::IfExpression) -> Vec<HirStmt> {
        let cond = self.lower_expr(&if_expr.condition);
        let then_body = if_expr.consequence.statements.iter()
            .flat_map(|s| self.lower_stmt(s))
            .collect();
        let else_body = if_expr.alternative.as_ref().map(|alt| {
            alt.statements.iter().flat_map(|s| self.lower_stmt(s)).collect()
        }).unwrap_or_default();
        vec![HirStmt::If { cond, then_body, else_body }]
    }

    // ── Expressions ───────────────────────────────────────────────────────────

    fn lower_expr(&mut self, expr: &Expression) -> HirExpr {
        match expr {
            Expression::Integer(i)  => HirExpr::LitInt(*i),
            Expression::Decimal(d)  => HirExpr::LitDecimal(*d),
            Expression::Boolean(b)  => HirExpr::LitBool(*b),
            Expression::String(s)   => HirExpr::LitStr(s.clone()),
            Expression::Null        => HirExpr::Null,

            Expression::Identifier(name) => {
                let ty = self.type_env.get(name).cloned().unwrap_or(SzType::Unknown);
                HirExpr::Var(name.clone(), ty)
            }

            Expression::Prefix(op, operand) => {
                let operand = self.lower_expr(operand);
                let ty = operand.ty();
                let hir_op = match op.as_str() {
                    "!" => HirUnaryOp::Not,
                    _   => HirUnaryOp::Neg,
                };
                HirExpr::UnaryOp { op: hir_op, operand: Box::new(operand), ty }
            }

            Expression::Infix(infix) => {
                // Null coalescing: a ?? b → if (a != null) a else b
                if infix.operator == "??" {
                    let left  = self.lower_expr(&infix.left);
                    let right = self.lower_expr(&infix.right);
                    let ty = left.ty();
                    let cond = HirExpr::BinOp {
                        op: HirBinOp::Ne,
                        left:  Box::new(left.clone()),
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

                let left  = self.lower_expr(&infix.left);
                let right = self.lower_expr(&infix.right);
                let op = self.map_binop(&infix.operator);
                let ty = self.binop_result_ty(&op, &left.ty(), &right.ty());
                HirExpr::BinOp { op, left: Box::new(left), right: Box::new(right), ty }
            }

            Expression::Call(call) => {
                let name = match call.function.as_ref() {
                    Expression::Identifier(n) => n.clone(),
                    _ => "__unknown".to_string(),
                };
                let args: Vec<HirExpr> = call.arguments.iter().map(|a| self.lower_expr(a)).collect();
                let ty = self.fn_sigs.get(&name)
                    .map(|(_, ret)| ret.clone())
                    .unwrap_or(SzType::Unknown);
                HirExpr::Call { name, args, ty }
            }

            Expression::DotCall(dc) => {
                let object = self.lower_expr(&dc.object);
                let args: Vec<HirExpr> = dc.arguments.iter().map(|a| self.lower_expr(a)).collect();

                // Optional chaining: obj?.method() → if (obj != null) obj.method() else null
                if dc.is_optional {
                    let cond = HirExpr::BinOp {
                        op: HirBinOp::Ne,
                        left:  Box::new(object.clone()),
                        right: Box::new(HirExpr::Null),
                        ty: SzType::Bool,
                    };
                    let call = HirExpr::MethodCall {
                        object: Box::new(object),
                        method: dc.method.clone(),
                        args,
                        ty: SzType::Unknown,
                    };
                    return HirExpr::If {
                        cond: Box::new(cond),
                        then_expr: Box::new(call),
                        else_expr: Box::new(HirExpr::Null),
                        ty: SzType::Null,
                    };
                }

                if dc.has_parens {
                    HirExpr::MethodCall {
                        object: Box::new(object), method: dc.method.clone(),
                        args, ty: SzType::Unknown,
                    }
                } else {
                    HirExpr::Field {
                        object: Box::new(object), name: dc.method.clone(),
                        ty: SzType::Unknown,
                    }
                }
            }

            // Ternary: cond ? a : b → HirExpr::If
            Expression::Ternary(t) => {
                let cond      = self.lower_expr(&t.condition);
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

            Expression::If(if_expr) => {
                // If used as expression: treat as conditional expression
                let cond = self.lower_expr(&if_expr.condition);
                let then_val = if_expr.consequence.statements.last()
                    .and_then(|s| if let Statement::Expression(e) = s { Some(self.lower_expr(e)) } else { None })
                    .unwrap_or(HirExpr::Null);
                let else_val = if_expr.alternative.as_ref()
                    .and_then(|alt| alt.statements.last())
                    .and_then(|s| if let Statement::Expression(e) = s { Some(self.lower_expr(e)) } else { None })
                    .unwrap_or(HirExpr::Null);
                let ty = then_val.ty();
                HirExpr::If {
                    cond: Box::new(cond),
                    then_expr: Box::new(then_val),
                    else_expr: Box::new(else_val),
                    ty,
                }
            }

            Expression::Index(idx) => {
                let array = self.lower_expr(&idx.left);
                let index = self.lower_expr(&idx.index);
                let ty = match array.ty() {
                    SzType::Array(t) => *t,
                    _ => SzType::Unknown,
                };
                HirExpr::Index { array: Box::new(array), index: Box::new(index), ty }
            }

            Expression::New(n) => {
                let args: Vec<HirExpr> = match &n.args {
                    NewArgs::Positional(args) => args.iter().map(|a| self.lower_expr(a)).collect(),
                    NewArgs::Fields(fields)   => fields.iter().map(|(_, v)| self.lower_expr(v)).collect(),
                };
                HirExpr::New { class: n.class_name.clone(), args }
            }

            Expression::ArrayLiteral(arr) => {
                let elements: Vec<HirExpr> = arr.elements.iter().map(|e| self.lower_expr(e)).collect();
                let elem_ty = arr.element_type.as_deref()
                    .map(SzType::from_annotation)
                    .unwrap_or_else(|| elements.first().map(|e| e.ty()).unwrap_or(SzType::Unknown));
                HirExpr::Array { elements, elem_ty }
            }

            // "Hello {name}!" → "Hello " + name.toString()
            Expression::InterpolatedString(parts) => {
                let exprs: Vec<HirExpr> = parts.iter().map(|p| match p {
                    StringPart::Literal(s) => HirExpr::LitStr(s.clone()),
                    StringPart::Expr(e) => HirExpr::MethodCall {
                        object: Box::new(self.lower_expr(e)),
                        method: "toString".to_string(),
                        args: vec![],
                        ty: SzType::Str,
                    },
                }).collect();

                exprs.into_iter().reduce(|acc, part| HirExpr::BinOp {
                    op: HirBinOp::Add,
                    left: Box::new(acc), right: Box::new(part),
                    ty: SzType::Str,
                }).unwrap_or(HirExpr::LitStr(String::new()))
            }

            // sizeof → constant integer at HIR level
            Expression::SizeOf(target) => {
                use crate::ast::SizeOfTarget;
                let size: i64 = match target {
                    SizeOfTarget::Type(name) => match name.as_str() {
                        "int" | "decimal" | "string" | "any" => 8,
                        "bool"                               => 1,
                        "null" | "void"                      => 0,
                        _                                    => 8,
                    },
                    SizeOfTarget::Expr(_) => 8, // conservative: pointer-sized at HIR
                };
                HirExpr::LitInt(size)
            }

            // Pointer expressions — stub as Null until native pointer support lands
            Expression::AddressOf(_) | Expression::Deref(_) => HirExpr::Null,

            // Phase 1: lambdas, dicts, spread, object-patch are unsupported
            Expression::FunctionLiteral(_) | Expression::Lambda(_)
            | Expression::DictLiteral(_)  | Expression::EntryLiteral(_, _)
            | Expression::ObjectPatch(_)  | Expression::Spread(_)
            | Expression::Match(_) => HirExpr::Null,
        }
    }

    // ── Helpers ───────────────────────────────────────────────────────────────

    fn map_binop(&self, op: &str) -> HirBinOp {
        match op {
            "+"  => HirBinOp::Add,  "-"  => HirBinOp::Sub,
            "*"  => HirBinOp::Mul,  "/"  => HirBinOp::Div,
            "%"  => HirBinOp::Mod,  "**" => HirBinOp::Pow,
            "==" => HirBinOp::Eq,   "!=" => HirBinOp::Ne,
            "<"  => HirBinOp::Lt,   "<=" => HirBinOp::Le,
            ">"  => HirBinOp::Gt,   ">=" => HirBinOp::Ge,
            "&&" => HirBinOp::And,  "||" => HirBinOp::Or,
            "&"  => HirBinOp::BitAnd, "|" => HirBinOp::BitOr,
            "^"  => HirBinOp::BitXor,
            "<<" => HirBinOp::Shl,  ">>" => HirBinOp::Shr,
            _    => HirBinOp::Add,
        }
    }

    fn binop_result_ty(&self, op: &HirBinOp, left: &SzType, right: &SzType) -> SzType {
        match op {
            HirBinOp::Eq | HirBinOp::Ne | HirBinOp::Lt | HirBinOp::Le
            | HirBinOp::Gt | HirBinOp::Ge | HirBinOp::And | HirBinOp::Or => SzType::Bool,
            _ => match (left, right) {
                (SzType::Str, _) | (_, SzType::Str)         => SzType::Str,
                (SzType::Decimal, _) | (_, SzType::Decimal) => SzType::Decimal,
                _                                            => left.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;

    // ── AST builder helpers ───────────────────────────────────────────────────

    fn program(stmts: Vec<ast::Statement>) -> ast::Program {
        ast::Program { statements: stmts }
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
            left: Box::new(l), operator: op.to_string(), right: Box::new(r),
            line: 0, column: 0,
        })
    }

    fn ident(name: &str) -> ast::Expression { ast::Expression::Identifier(name.to_string()) }

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
                parameters: params.iter().map(|(n, t)| ast::Parameter {
                    name: n.to_string(),
                    type_name: Some(t.to_string()),
                    is_rest: false,
                    default_value: None,
                }).collect(),
                body: block(body),
                is_generator: false,
            },
        })
    }

    fn main_fn(hir: &crate::compiler::hir::HirProgram) -> &crate::compiler::hir::HirFunction {
        hir.functions.iter().find(|f| f.name == "__sz_main").expect("no __sz_main")
    }

    // ── Let / Assign ─────────────────────────────────────────────────────────

    #[test]
    fn let_integer_lowers_to_hir_let() {
        let hir = HirLowerer::new().lower_program(&program(vec![let_int("x", 99)]));
        let m = main_fn(&hir);
        assert_eq!(m.body.len(), 1);
        match &m.body[0] {
            HirStmt::Let { name, ty, value, .. } => {
                assert_eq!(name, "x");
                assert_eq!(*ty, SzType::Int);
                assert!(matches!(value, HirExpr::LitInt(99)));
            }
            s => panic!("expected Let, got {:?}", s),
        }
    }

    #[test]
    fn let_bool_infers_bool_type() {
        let hir = HirLowerer::new().lower_program(&program(vec![let_bool("flag", false)]));
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
        let hir = HirLowerer::new().lower_program(&program(vec![
            let_int("a", 1),
            let_int("b", 2),
            let_int("c", 3),
        ]));
        let m = main_fn(&hir);
        assert_eq!(m.body.len(), 3);
    }

    // ── Arithmetic / BinOp ───────────────────────────────────────────────────

    #[test]
    fn addition_becomes_binop_add() {
        let expr = infix(ast::Expression::Integer(3), "+", ast::Expression::Integer(4));
        let hir = HirLowerer::new().lower_program(&program(vec![
            ast::Statement::Let(ast::LetStatement { name: "r".into(), value: expr, is_const: false }),
        ]));
        match &main_fn(&hir).body[0] {
            HirStmt::Let { value: HirExpr::BinOp { op, ty, .. }, .. } => {
                assert_eq!(*op, HirBinOp::Add);
                assert_eq!(*ty, SzType::Int);
            }
            s => panic!("{:?}", s),
        }
    }

    #[test]
    fn comparison_produces_bool_type() {
        let expr = infix(ident("x"), "<", ast::Expression::Integer(10));
        let hir = HirLowerer::new().lower_program(&program(vec![
            ast::Statement::Let(ast::LetStatement { name: "c".into(), value: expr, is_const: false }),
        ]));
        match &main_fn(&hir).body[0] {
            HirStmt::Let { ty, value: HirExpr::BinOp { op, .. }, .. } => {
                assert_eq!(*ty, SzType::Bool);
                assert_eq!(*op, HirBinOp::Lt);
            }
            s => panic!("{:?}", s),
        }
    }

    // ── Control flow ─────────────────────────────────────────────────────────

    #[test]
    fn out_statement_lowers_correctly() {
        let hir = HirLowerer::new().lower_program(&program(vec![
            out(ast::Expression::Integer(42)),
        ]));
        assert!(matches!(&main_fn(&hir).body[0], HirStmt::Out(HirExpr::LitInt(42))));
    }

    #[test]
    fn while_loop_lowers_to_hir_while() {
        let w = ast::Statement::While(ast::WhileStatement {
            condition: ast::Expression::Boolean(true),
            body: block(vec![ast::Statement::Break]),
            label: None,
        });
        let hir = HirLowerer::new().lower_program(&program(vec![w]));
        let stmt = &main_fn(&hir).body[0];
        assert!(matches!(stmt, HirStmt::While { .. }));
        if let HirStmt::While { cond, body } = stmt {
            assert!(matches!(cond, HirExpr::LitBool(true)));
            assert!(matches!(body[0], HirStmt::Break));
        }
    }

    #[test]
    fn break_and_continue_lower_directly() {
        let hir = HirLowerer::new().lower_program(&program(vec![
            ast::Statement::Break,
            ast::Statement::Continue,
        ]));
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
        let hir = HirLowerer::new().lower_program(&program(vec![if_stmt]));
        let m = main_fn(&hir);
        match &m.body[0] {
            HirStmt::If { cond, then_body, else_body } => {
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
        let hir = HirLowerer::new().lower_program(&program(vec![dw]));
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
        let hir = HirLowerer::new().lower_program(&program(vec![
            ast::Statement::Let(ast::LetStatement { name: "v".into(), value: ternary, is_const: false }),
        ]));
        match &main_fn(&hir).body[0] {
            HirStmt::Let { value: HirExpr::If { .. }, .. } => {}
            s => panic!("expected Let with HirExpr::If, got {:?}", s),
        }
    }

    #[test]
    fn null_coalescing_desugars_to_hir_if_expr() {
        let nc = infix(ident("maybe"), "??", ast::Expression::Integer(0));
        let hir = HirLowerer::new().lower_program(&program(vec![
            ast::Statement::Let(ast::LetStatement { name: "v".into(), value: nc, is_const: false }),
        ]));
        match &main_fn(&hir).body[0] {
            HirStmt::Let { value: HirExpr::If { .. }, .. } => {}
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
        let hir = HirLowerer::new().lower_program(&program(vec![sw]));
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
        let hir = HirLowerer::new().lower_program(&program(vec![
            fn_decl("add", vec![("a", "int"), ("b", "int")], "int", vec![
                ast::Statement::Return(ast::ReturnStatement {
                    return_value: infix(ident("a"), "+", ident("b")),
                }),
            ]),
        ]));
        let f = hir.functions.iter().find(|f| f.name == "add").unwrap();
        assert_eq!(f.params.len(), 2);
        assert_eq!(f.params[0].ty, SzType::Int);
        assert_eq!(f.params[1].ty, SzType::Int);
        assert_eq!(f.ret_type, SzType::Int);
        assert!(matches!(f.body[0], HirStmt::Return(Some(_))));
    }

    #[test]
    fn function_void_return_is_void() {
        let hir = HirLowerer::new().lower_program(&program(vec![
            fn_decl("greet", vec![], "void", vec![
                out(ast::Expression::String("hi".to_string())),
            ]),
        ]));
        let f = hir.functions.iter().find(|f| f.name == "greet").unwrap();
        assert_eq!(f.ret_type, SzType::Void);
        assert_eq!(f.params.len(), 0);
    }

    #[test]
    fn foreach_desugars_to_block_with_for_loop() {
        let fe = ast::Statement::ForEach(ast::ForEachStatement {
            var: ast::ForEachVar::Name("n".to_string()),
            iterable: ident("items"),
            body: block(vec![out(ident("n"))]),
            label: None,
        });
        let hir = HirLowerer::new().lower_program(&program(vec![fe]));
        match &main_fn(&hir).body[0] {
            HirStmt::Block(stmts) => {
                // let_iter + let_len + For
                assert!(stmts.len() >= 3, "expected let_iter, let_len, For");
                assert!(matches!(stmts[2], HirStmt::For { .. }));
            }
            s => panic!("expected Block from foreach, got {:?}", s),
        }
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
                        line: 0, column: 0,
                    }),
                    "+",
                    ast::Expression::Call(ast::CallExpression {
                        function: Box::new(ident("fib")),
                        arguments: vec![infix(ident("n"), "-", ast::Expression::Integer(2))],
                        line: 0, column: 0,
                    }),
                ),
            }),
        ];
        let hir = HirLowerer::new().lower_program(&program(vec![
            fn_decl("fib", vec![("n", "int")], "int", body),
        ]));
        let f = hir.functions.iter().find(|f| f.name == "fib").unwrap();
        assert_eq!(f.ret_type, SzType::Int);
        assert_eq!(f.params[0].name, "n");
        // body: If + Return
        assert_eq!(f.body.len(), 2);
        assert!(matches!(f.body[0], HirStmt::If { .. }));
        assert!(matches!(f.body[1], HirStmt::Return(Some(_))));
    }
}
