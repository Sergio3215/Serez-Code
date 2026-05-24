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

            Statement::Block(b) => {
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

                self.type_env.insert(iter_name.clone(), iter_ty.clone());
                self.type_env.insert(idx_name.clone(), SzType::Int);
                self.type_env.insert(len_name.clone(), SzType::Int);
                self.type_env.insert(fe.var_name.clone(), elem_ty.clone());

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
                    name: fe.var_name.clone(), ty: elem_ty.clone(),
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

            // Phase 1: lambdas, dicts, spread, object-patch are unsupported
            Expression::FunctionLiteral(_) | Expression::Lambda(_)
            | Expression::DictLiteral(_)  | Expression::EntryLiteral(_, _)
            | Expression::ObjectPatch(_)  | Expression::Spread(_) => HirExpr::Null,
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
