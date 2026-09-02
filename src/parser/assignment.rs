//! Writing to something: plain assignment, compound assignment, and the three
//! nested forms that decide what a write actually lands on.
//!
//! **This is the most behaviour-sensitive module in the parser**, and it is worth
//! saying why before anyone edits it. Serez has value semantics: an object
//! assigned or passed is copied. Receiver writeback is the exception that makes
//! `a[0].push(x)` and `obj.field.mutate()` visible to the original rather than to
//! a dropped copy — and `MATURITY_AUDIT.md` records that the last defect here
//! was silent data loss, found only because `serez-agentai` misbehaved.
//! `serez-ui` leans on this behaviour harder than on anything else in the
//! language, which is why the ecosystem canary is the gate that matters for
//! changes to this file.
//!
//! The shape the parser must get right is `is_writable_chain`. Only a chain of
//! *reads* — `a`, `a.b`, `a[i]`, `a.b[i].c` — can receive a nested write,
//! because a call in the middle (`a.give().c`) produces a temporary and writing
//! to it is invisible to everyone. The evaluator re-walks that same shape to
//! find the slot, so the two have to agree; changing the predicate here without
//! changing the evaluator would corrupt writes rather than reject them.
//!
//! `parse_expression_statement` is the large one because a statement beginning
//! with an identifier is ambiguous until quite late: it may be an assignment, a
//! compound assignment, an index assignment, a nested field assignment, or just
//! an expression. It resolves that by parsing the expression first and then
//! asking what shape came back — which is why the `try_build_*` helpers take an
//! already-parsed `Expression` rather than reading tokens.

use super::{Parser, Precedence};
use crate::ast::*;
use crate::span::Span;
use crate::token::TokenType;

/// ¿La expresión es una cadena de LECTURAS (`a`, `a.b`, `a[i]`, `a.b[i].c`)?
/// Sólo esas sirven como receptor de una asignación anidada: una llamada
/// (`a.dame().c`) produce un temporal, y escribirle no se ve desde ningún lado.
/// El evaluador vuelve a recorrer la misma forma para encontrar el slot.
fn is_writable_chain(e: &Expression) -> bool {
    match e {
        Expression::Identifier { name: _, .. } => true,
        Expression::DotCall(d) if d.arguments.is_empty() && !d.has_parens => {
            is_writable_chain(&d.object)
        }
        Expression::Index(ix) => is_writable_chain(&ix.left),
        _ => false,
    }
}

impl Parser {
    pub(super) fn parse_expression_statement(&mut self) -> Option<Statement> {
        // Where the statement begins, before `parse_expression` moves the
        // cursor. Every assignment form below reaches from here to the cursor.
        let open = self.current_token.span;
        let expr = self.parse_expression(Precedence::Lowest)?;

        let is_assign = self.peek_token.token_type == TokenType::Assign;
        let is_compound = self.is_compound_assign(&self.peek_token.token_type);

        if is_assign || is_compound {
            // *ptr = val
            if is_assign {
                if let Expression::Deref(ref ptr_expr) = expr {
                    let ptr_clone = ptr_expr.clone();
                    self.next_token(); // consume '='
                    self.next_token(); // first token of rhs
                    let value = self.parse_expression(Precedence::Lowest)?;
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    return Some(Statement::DerefAssign {
                        ptr: ptr_clone,
                        value,
                    });
                }
            }

            // obj.field = val  or  obj.field += val
            if let Expression::DotCall(ref dot) = expr {
                if dot.arguments.is_empty() {
                    if let Expression::Identifier {
                        name: ref obj_name, ..
                    } = *dot.object
                    {
                        let object = obj_name.clone();
                        let field = dot.method.clone();
                        let line = dot.span.line;
                        let column = dot.span.column;
                        let op_str = if is_compound {
                            Some(Self::compound_op(&self.peek_token.token_type).to_string())
                        } else {
                            None
                        };
                        self.next_token(); // '=' or compound token
                        self.next_token(); // first token of rhs
                        let rhs = self.parse_expression(Precedence::Lowest)?;
                        let value = if let Some(op) = op_str {
                            Expression::Infix(InfixExpression {
                                left: Box::new(Expression::DotCall(DotCallExpression {
                                    object: Box::new(Expression::Identifier {
                                        name: object.clone(),
                                        span: dot.span,
                                    }),
                                    method: field.clone(),
                                    arguments: vec![],
                                    has_parens: false,
                                    is_optional: false,
                                    span: Span::point(line, column),
                                })),
                                operator: op,
                                right: Box::new(rhs),
                                span: Span::point(line, column),
                            })
                        } else {
                            rhs
                        };
                        if self.peek_token.token_type == TokenType::Semicolon {
                            self.next_token();
                        }
                        return Some(Statement::FieldAssign(FieldAssignStatement {
                            object,
                            field,
                            value,
                            span: self.span_to_here(open),
                        }));
                    }

                    if let Some(st) = self.try_build_nested_field_assign(dot, is_compound, open) {
                        return Some(st);
                    }
                }
            }

            // expr[idx] = val  or  expr[idx] += val
            if let Expression::Index(_) = &expr {
                if is_assign {
                    return self.try_build_index_assign(expr, open);
                } else {
                    return self.try_build_index_compound_assign(expr, open);
                }
            }
        }

        // obj.field++  /  this.field++  /  obj.field--  /  this.field--
        // Also catches Index targets arriving via expression_statement path (e.g. this.arr[i]++)
        let is_incr = self.peek_token.token_type == TokenType::PlusPlus;
        let is_decr = self.peek_token.token_type == TokenType::MinusMinus;
        if is_incr || is_decr {
            let op = if is_incr { "+" } else { "-" };
            let line = self.current_token.line;
            let column = self.current_token.column;

            if let Expression::DotCall(ref dot) = expr {
                if dot.arguments.is_empty() {
                    if let Expression::Identifier {
                        name: ref obj_name, ..
                    } = *dot.object
                    {
                        let object = obj_name.clone();
                        let field = dot.method.clone();
                        let dline = dot.span.line;
                        let dcol = dot.span.column;
                        self.next_token(); // ++ or --
                        if self.peek_token.token_type == TokenType::Semicolon {
                            self.next_token();
                        }
                        let value = Expression::Infix(InfixExpression {
                            left: Box::new(Expression::DotCall(DotCallExpression {
                                object: Box::new(Expression::Identifier {
                                    name: object.clone(),
                                    span: dot.span,
                                }),
                                method: field.clone(),
                                arguments: vec![],
                                has_parens: false,
                                is_optional: false,
                                span: Span::point(dline, dcol),
                            })),
                            operator: op.to_string(),
                            right: Box::new(Expression::Integer {
                                value: 1,
                                span: Span::point(line, column),
                            }),
                            span: Span::point(line, column),
                        });
                        return Some(Statement::FieldAssign(FieldAssignStatement {
                            object,
                            field,
                            value,
                            span: self.span_to_here(open),
                        }));
                    }
                }
            }

            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                let value = Expression::Infix(InfixExpression {
                    left: Box::new(expr.clone()),
                    operator: op.to_string(),
                    right: Box::new(Expression::Integer {
                        value: 1,
                        span: Span::point(line, column),
                    }),
                    span: Span::point(line, column),
                });
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                    span: self.span_to_here(open),
                }));
            }
        }

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Expression(expr))
    }

    pub(super) fn parse_index_assign_or_expr_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.is_compound_assign(&self.peek_token.token_type) {
            return self.try_build_index_compound_assign(expr, open);
        }
        // arr[i]++  /  arr[i]--
        if matches!(
            self.peek_token.token_type,
            TokenType::PlusPlus | TokenType::MinusMinus
        ) {
            if let Expression::Index(ref idx_expr) = expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                let line = self.current_token.line;
                let column = self.current_token.column;
                let op = if self.peek_token.token_type == TokenType::PlusPlus {
                    "+"
                } else {
                    "-"
                };
                self.next_token(); // ++ or --
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                let value = Expression::Infix(InfixExpression {
                    left: Box::new(expr),
                    operator: op.to_string(),
                    right: Box::new(Expression::Integer {
                        value: 1,
                        span: Span::point(line, column),
                    }),
                    span: Span::point(line, column),
                });
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                    span: self.span_to_here(open),
                }));
            }
        }
        self.try_build_index_assign(expr, open)
    }

    pub(super) fn try_build_index_assign(
        &mut self,
        expr: Expression,
        open: Span,
    ) -> Option<Statement> {
        let is_assign = self.peek_token.token_type == TokenType::Assign;
        let is_compound = self.is_compound_assign(&self.peek_token.token_type);
        if is_assign {
            if let Expression::Index(idx_expr) = &expr {
                let target = (*idx_expr.left).clone();
                let index = (*idx_expr.index).clone();
                self.next_token(); // '='
                self.next_token(); // first token of value
                let value = self.parse_expression(Precedence::Lowest)?;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                return Some(Statement::IndexAssign(IndexAssignStatement {
                    target,
                    index,
                    value,
                    span: self.span_to_here(open),
                }));
            }
        }
        // `objs[1].campo = x` entra por acá (la sentencia arranca con `ident[`,
        // no por parse_expression_statement) y termina siendo un DotCall, no un
        // Index. Sin esto el '=' quedaba sin consumir y era un error de parseo.
        if is_assign || is_compound {
            if let Expression::DotCall(ref dot) = expr {
                if let Some(st) = self.try_build_nested_field_assign(dot, is_compound, open) {
                    return Some(st);
                }
            }
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Expression(expr))
    }

    /// `a.b.c = val` (y su forma compuesta) — el receptor es una CADENA, no un
    /// nombre suelto, así que no entra en `FieldAssignStatement`. Antes moría en
    /// el parser con "Unexpected token '='" y había que rearmar y reasignar el
    /// objeto intermedio entero.
    ///
    /// Sólo una cadena de lecturas es destino válido: `a.dame().c = x` escribe
    /// sobre un temporal y no se vería desde ningún lado. Devuelve `None` sin
    /// consumir nada si la forma no aplica, para que el llamador siga probando.
    pub(super) fn try_build_nested_field_assign(
        &mut self,
        dot: &DotCallExpression,
        is_compound: bool,
        open: Span,
    ) -> Option<Statement> {
        if dot.has_parens || !dot.arguments.is_empty() {
            return None;
        }
        if !is_writable_chain(&dot.object) {
            return None;
        }
        // Un solo salto sobre una variable ya lo cubre FieldAssign.
        if matches!(*dot.object, Expression::Identifier { name: _, .. }) {
            return None;
        }

        let object = (*dot.object).clone();
        let field = dot.method.clone();
        let line = dot.span.line;
        let column = dot.span.column;
        let op_str = if is_compound {
            Some(Self::compound_op(&self.peek_token.token_type).to_string())
        } else {
            None
        };
        self.next_token(); // '=' or compound token
        self.next_token(); // first token of rhs
        let rhs = self.parse_expression(Precedence::Lowest)?;
        let value = if let Some(op) = op_str {
            Expression::Infix(InfixExpression {
                left: Box::new(Expression::DotCall(DotCallExpression {
                    object: Box::new(object.clone()),
                    method: field.clone(),
                    arguments: vec![],
                    has_parens: false,
                    is_optional: false,
                    span: Span::point(line, column),
                })),
                operator: op,
                right: Box::new(rhs),
                span: Span::point(line, column),
            })
        } else {
            rhs
        };
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::NestedFieldAssign(NestedFieldAssignStatement {
            object,
            field,
            value,
            span: self.span_to_here(open),
        }))
    }

    /// Desugar `arr[i] += rhs` → `arr[i] = arr[i] + rhs`
    pub(super) fn try_build_index_compound_assign(
        &mut self,
        expr: Expression,
        open: Span,
    ) -> Option<Statement> {
        if let Expression::Index(ref idx_expr) = expr {
            let target = (*idx_expr.left).clone();
            let index = (*idx_expr.index).clone();
            let line = self.current_token.line;
            let column = self.current_token.column;
            let op = Self::compound_op(&self.peek_token.token_type).to_string();
            self.next_token(); // compound token
            self.next_token(); // first token of rhs
            let rhs = self.parse_expression(Precedence::Lowest)?;
            let value = Expression::Infix(InfixExpression {
                left: Box::new(expr.clone()),
                operator: op,
                right: Box::new(rhs),
                span: Span::point(line, column),
            });
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::IndexAssign(IndexAssignStatement {
                target,
                index,
                value,
                span: self.span_to_here(open),
            }));
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Expression(expr))
    }

    /// Desugar `x += rhs` → `x = x + rhs`
    pub(super) fn parse_compound_assign_statement(&mut self) -> Option<Statement> {
        let name = self.current_token.literal.clone();
        let open = self.current_token.span;
        let line = self.current_token.line;
        let column = self.current_token.column;
        let op = Self::compound_op(&self.peek_token.token_type).to_string();
        self.next_token(); // compound token
        self.next_token(); // first token of rhs
        let rhs = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        let value = Expression::Infix(InfixExpression {
            left: Box::new(Expression::Identifier {
                name: name.clone(),
                span: Span::point(line, column),
            }),
            operator: op,
            right: Box::new(rhs),
            span: Span::point(line, column),
        });
        Some(Statement::Assign(AssignStatement {
            name,
            value,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn parse_assign_statement(&mut self) -> Option<Statement> {
        let name = self.current_token.literal.clone();
        let open = self.current_token.span;
        self.next_token(); // '='
        self.next_token(); // first token of value

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Assign(AssignStatement {
            name,
            value,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn is_compound_assign(&self, tt: &TokenType) -> bool {
        matches!(
            tt,
            TokenType::PlusEq
                | TokenType::MinusEq
                | TokenType::StarEq
                | TokenType::SlashEq
                | TokenType::PercentEq
        )
    }

    pub(super) fn compound_op(tt: &TokenType) -> &'static str {
        match tt {
            TokenType::PlusEq => "+",
            TokenType::MinusEq => "-",
            TokenType::StarEq => "*",
            TokenType::SlashEq => "/",
            TokenType::PercentEq => "%",
            _ => unreachable!(),
        }
    }
}
