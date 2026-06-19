#![allow(unused_imports)]
use crate::ast::{self, Expression, Statement};
use crate::region::{ObjectData, ObjectRef, OwnedValue, RegionId};
use crate::scope::ScopeStack;
use std::collections::{HashMap, HashSet};
use std::io::{self, Write};
use std::rc::Rc;
use super::{EvalResult, StoredClass, CallFrame, type_matches, obj_data_to_key_str,
            obj_data_eq, format_decimal, json_stringify_owned, json_parse,
            operator_to_method_name};

impl super::Evaluator {
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

    pub(super) fn analyze_function(&self, name: &str, func: &ast::FunctionLiteral, total: &mut usize) {
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
                ast::Statement::ForEach(fe) => {
                    local_mem += 8; // iteration variable
                    local_mem += self.estimate_expression(&fe.iterable);
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

    pub(super) fn estimate_expression(&self, expr: &ast::Expression) -> usize {
        match expr {
            ast::Expression::Integer(_) => 8,
            ast::Expression::Decimal(_) => 8,
            ast::Expression::Dec(_) => 16,
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
            ast::Expression::New(n) => {
                let arg_cost: usize = match &n.args {
                    ast::NewArgs::Positional(args) => args.iter().map(|e| self.estimate_expression(e)).sum(),
                    ast::NewArgs::Fields(fields) => fields.iter().map(|(_, e)| self.estimate_expression(e)).sum(),
                };
                32 + arg_cost
            }
            ast::Expression::ObjectPatch(fields) => {
                32 + fields.iter().map(|(_, e)| self.estimate_expression(e)).sum::<usize>()
            }
            ast::Expression::Ternary(t) => {
                self.estimate_expression(&t.condition)
                    + std::cmp::max(
                        self.estimate_expression(&t.then_expr),
                        self.estimate_expression(&t.else_expr),
                    )
            }
            ast::Expression::Spread(inner) => self.estimate_expression(inner),
            ast::Expression::SizeOf(_)    => 8,
            ast::Expression::AddressOf(inner) => 8 + self.estimate_expression(inner),
            ast::Expression::Deref(inner)     => 8 + self.estimate_expression(inner),
            ast::Expression::Match(m) => {
                let subject_cost = self.estimate_expression(&m.subject);
                let arms_cost: usize = m.arms.iter().map(|arm| {
                    arm.body.statements.iter().filter_map(|s| {
                        if let ast::Statement::Expression(e) = s { Some(self.estimate_expression(e)) }
                        else { None }
                    }).sum::<usize>()
                }).max().unwrap_or(0);
                subject_cost + arms_cost
            }
            ast::Expression::UnsafeBlock(_) => 32,
        }
    }

}
