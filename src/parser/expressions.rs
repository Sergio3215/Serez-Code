//! Expressions: the precedence table, the prefix dispatcher, and the infix loop.
//!
//! **This algorithm is deliberately unchanged.** `Precedence` +
//! `token_precedence` + `parse_expression(precedence)` + `parse_infix_chain`
//! already form a precedence-climbing parser rather than naive recursive
//! descent, and M1's charter is explicit that moving it toward anything else —
//! Pratt parsing, a table-driven loop — is a separate project needing
//! differential testing, because it can alter precedence or associativity
//! without altering a single test's printed output. `spec/operators.md` is the
//! normative contract for the table below; nothing here may drift from it.
//!
//! `parse_expression` is the prefix half: given the token the cursor is on, build
//! the smallest complete expression starting there. It is one large `match`
//! because that is what a prefix dispatcher is, and it is the reason this file
//! needs a large stack in debug builds — an unoptimized frame for it measures
//! around 8 KiB, which is what `MAX_PARSE_DEPTH` is sized against.
//!
//! `parse_infix_chain` is the other half, and the subtle one. It appends
//! operators in a *flat loop* rather than recursing, so `1 + 1 + 1 + …` never
//! troubles the parser's own stack — but it still builds a left-leaning tree one
//! level deeper per operator, and the type checker, the evaluator and the AST's
//! drop glue each recurse once per level of that tree. So it charges the depth
//! guard once per operator rather than once per call: see `depth.rs`, and
//! `tests/err_parse_depth_chain.sz`, which exists to prove exactly that.
//!
//! `new`, `sizeof` and the argument list live here because they are prefix forms
//! the dispatcher reaches directly, not because they have anything else in
//! common.

use super::literals::{parse_dec_literal, parse_interpolated_string};
use super::types::is_type_keyword;
use super::{DepthGuard, Parser};
use crate::ast::*;
use crate::token::TokenType;

#[derive(PartialEq, PartialOrd)]
pub enum Precedence {
    Lowest,
    Pipe,         // |>
    Ternary,      // ? :
    NullCoalesce, // ??
    LogicalOr,    // ||
    LogicalAnd,   // &&
    BitOr,        // |
    BitXor,       // ^
    BitAnd,       // &
    Equals,       // ==
    LessGreater,  // > or <
    Shift,        // << >>
    Sum,          // +
    Product,      // *
    Power,        // **
    Prefix,       // -X or !X
    Call,         // myFunction(X)
    Index,        // array[index]
}

pub fn token_precedence(token_type: &TokenType) -> Precedence {
    match token_type {
        TokenType::Pipe => Precedence::Pipe,
        TokenType::Question => Precedence::Ternary,
        TokenType::NullCoalesce => Precedence::NullCoalesce,
        TokenType::Or => Precedence::LogicalOr,
        TokenType::And => Precedence::LogicalAnd,
        TokenType::BitOr => Precedence::BitOr,
        TokenType::BitXor => Precedence::BitXor,
        TokenType::BitAnd => Precedence::BitAnd,
        TokenType::Eq | TokenType::NotEq => Precedence::Equals,
        TokenType::KwIs => Precedence::LessGreater,
        TokenType::Lt | TokenType::Gt | TokenType::LtEq | TokenType::GtEq => {
            Precedence::LessGreater
        }
        TokenType::Shl | TokenType::Shr => Precedence::Shift,
        TokenType::Plus | TokenType::Minus => Precedence::Sum,
        TokenType::Slash | TokenType::Asterisk | TokenType::Percent => Precedence::Product,
        TokenType::Power => Precedence::Power,
        TokenType::LParen => Precedence::Call,
        TokenType::Dot | TokenType::QuestionDot => Precedence::Call,
        TokenType::LBracket => Precedence::Index,
        _ => Precedence::Lowest,
    }
}

impl Parser {
    pub(super) fn parse_expression(&mut self, precedence: Precedence) -> Option<Expression> {
        // Where this expression begins. Every node built below reaches from
        // here to wherever the cursor has got to.
        let open = self.current_token.span;
        // Every nested sub-expression re-enters here, so this is the one place
        // that has to hold the line against runaway nesting. See MAX_PARSE_DEPTH.
        let _depth = self.enter_depth()?;
        // ── PREFIX ────────────────────────────────────────────────────────────
        let left_exp = match self.current_token.token_type {
            // Single-param lambda: item => body
            TokenType::Ident if self.peek_token.token_type == TokenType::Arrow => {
                let param = self.current_token.literal.clone();
                self.next_token(); // consume '=>'
                let body = self.parse_lambda_body()?;
                Some(Expression::Lambda(LambdaExpression {
                    params: vec![param],
                    body,
                    span: self.span_to_here(open),
                }))
            }

            TokenType::Ident => Some(Expression::Identifier {
                name: self.current_token.literal.clone(),
                span: self.current_token.span,
            }),

            TokenType::Int => {
                if let Ok(num) = self.current_token.literal.parse::<i64>() {
                    Some(Expression::Integer(num))
                } else {
                    self.parser_error(&format!(
                        "Invalid integer literal '{}' (out of 64-bit range?)",
                        self.current_token.literal
                    ));
                    None
                }
            }

            TokenType::Decimal => {
                if let Ok(num) = self.current_token.literal.parse::<f64>() {
                    Some(Expression::Decimal(num))
                } else {
                    self.parser_error(&format!(
                        "Invalid decimal literal '{}'",
                        self.current_token.literal
                    ));
                    None
                }
            }

            TokenType::Dec => match parse_dec_literal(&self.current_token.literal) {
                Some(d) => Some(Expression::Dec(d)),
                None => {
                    self.parser_error(&format!(
                        "Invalid dec literal '{}'",
                        self.current_token.literal
                    ));
                    None
                }
            },

            TokenType::String => {
                let s = self.current_token.literal.clone();
                if s.contains('{') {
                    let parsed = parse_interpolated_string(&s, self.source_name.as_deref());
                    if parsed.is_none() {
                        self.had_error.set(true);
                    }
                    parsed
                } else {
                    // Replace \{ sentinel (\x01) with literal { in non-interpolated strings
                    Some(Expression::String(s.replace('\x01', "{")))
                }
            }

            // Raw string r"..." — already literal (braces not interpolated).
            TokenType::RawString => Some(Expression::String(self.current_token.literal.clone())),

            TokenType::True => Some(Expression::Boolean(true)),
            TokenType::False => Some(Expression::Boolean(false)),
            TokenType::KwNull => Some(Expression::Null),

            TokenType::Bang | TokenType::Minus | TokenType::BitNot => {
                let operator = self.current_token.literal.clone();
                self.next_token();
                let right = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Prefix(operator, Box::new(right)))
            }

            // &varname — address-of
            TokenType::BitAnd => {
                self.next_token();
                let inner = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::AddressOf(Box::new(inner)))
            }

            // *ptr — dereference
            TokenType::Asterisk => {
                self.next_token();
                let inner = self.parse_expression(Precedence::Prefix)?;
                Some(Expression::Deref(Box::new(inner)))
            }

            // sizeof(type | expr)
            TokenType::KwSizeof => self.parse_sizeof_expression(),

            // Zero-param lambda: () => body
            TokenType::LParen if self.peek_token.token_type == TokenType::RParen => {
                self.next_token(); // consume ')'
                if self.peek_token.token_type == TokenType::Arrow {
                    self.next_token(); // consume '=>'
                    let body = self.parse_lambda_body()?;
                    Some(Expression::Lambda(LambdaExpression {
                        params: vec![],
                        body,
                        span: self.span_to_here(open),
                    }))
                } else {
                    self.parser_error("Empty parentheses '()' are not a valid expression");
                    None
                }
            }

            // Multi-param lambda: (a, b) => body  /  (a) => body  /  (expr)
            TokenType::LParen if self.peek_token.token_type == TokenType::Ident => {
                self.next_token(); // consume '(' → current = first ident
                let first_name = self.current_token.literal.clone();
                let first_name_span = self.current_token.span;

                match self.peek_token.token_type {
                    // (a, b, ...) => body
                    TokenType::Comma => {
                        let mut params = vec![first_name];
                        while self.peek_token.token_type == TokenType::Comma {
                            self.next_token(); // ','
                            self.next_token(); // next ident
                            if self.current_token.token_type != TokenType::Ident {
                                self.parser_error("Expected identifier in lambda parameters");
                                return None;
                            }
                            params.push(self.current_token.literal.clone());
                        }
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' after lambda parameters");
                            return None;
                        }
                        self.next_token(); // ')'
                        if self.peek_token.token_type != TokenType::Arrow {
                            self.parser_error("Expected '=>' after lambda parameters");
                            return None;
                        }
                        self.next_token(); // '=>'
                        let body = self.parse_lambda_body()?;
                        Some(Expression::Lambda(LambdaExpression {
                            params,
                            body,
                            span: self.span_to_here(open),
                        }))
                    }

                    // (a) => body  or  just (a)
                    TokenType::RParen => {
                        self.next_token(); // ')'
                        if self.peek_token.token_type == TokenType::Arrow {
                            self.next_token(); // '=>'
                            let body = self.parse_lambda_body()?;
                            Some(Expression::Lambda(LambdaExpression {
                                params: vec![first_name],
                                body,
                                span: self.span_to_here(open),
                            }))
                        } else {
                            Some(Expression::Identifier {
                                name: first_name,
                                span: first_name_span,
                            })
                        }
                    }

                    // (x => body) — single-param lambda wrapped in parentheses.
                    // (x) => body is handled by the RParen arm; this is the case
                    // where the param has no inner parens: ( x => ... ). (B-84)
                    TokenType::Arrow => {
                        self.next_token(); // current = '=>'
                        let body = self.parse_lambda_body()?;
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' after parenthesized lambda");
                            return None;
                        }
                        self.next_token(); // ')'
                        Some(Expression::Lambda(LambdaExpression {
                            params: vec![first_name],
                            body,
                            span: self.span_to_here(open),
                        }))
                    }

                    // (ident op ...) — grouped expression starting with an identifier
                    _ => {
                        let first = Some(Expression::Identifier {
                            name: first_name,
                            span: first_name_span,
                        });
                        let inner = self.parse_infix_chain(first, Precedence::Lowest)?;
                        if self.peek_token.token_type != TokenType::RParen {
                            self.parser_error("Expected ')' in grouped expression");
                            return None;
                        }
                        self.next_token(); // ')'
                        Some(inner)
                    }
                }
            }

            // Regular grouped expression: (expr)
            TokenType::LParen => {
                self.next_token();
                let exp = self.parse_expression(Precedence::Lowest);
                if self.peek_token.token_type != TokenType::RParen {
                    return None;
                }
                self.next_token();
                exp
            }

            TokenType::LBracket => self.parse_array_literal(),
            TokenType::LBrace => self.parse_brace_expression(),
            TokenType::If => self.parse_if_expression(),
            TokenType::KwNew => self.parse_new_expression(),

            TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
            | TokenType::KwDec
            | TokenType::KwString
            | TokenType::KwBool
            | TokenType::KwAny => self.parse_arrow_function(),

            TokenType::Function => {
                let mut return_type = None;
                if is_type_keyword(&self.peek_token.token_type) {
                    self.next_token();
                    return_type = Some(self.current_token.literal.clone());
                }

                if self.peek_token.token_type != TokenType::LParen {
                    return None;
                }
                self.next_token();

                let parameters = self.parse_function_parameters()?;

                if self.peek_token.token_type != TokenType::LBrace {
                    return None;
                }
                self.next_token();

                let body_stmt = self.parse_block_statement()?;
                let body = match body_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                };

                Some(Expression::FunctionLiteral(FunctionLiteral {
                    return_type,
                    parameters,
                    body,
                    is_generator: false,
                    span: self.span_to_here(open),
                }))
            }

            TokenType::KwMatch => self.parse_match_expression(),

            // unsafe { ... } as an expression (returns last value of block)
            TokenType::KwUnsafe => {
                self.next_token(); // consume 'unsafe'
                if self.current_token.token_type != TokenType::LBrace {
                    self.had_error.set(true);
                    eprintln!("❌ PARSE ERROR: expected '{{' after 'unsafe'");
                    return None;
                }
                let block_stmt = self.parse_block_statement()?;
                let block = match block_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                };
                Some(Expression::UnsafeBlock(block))
            }

            TokenType::Illegal => None, // the lexer already emitted an SZ1xxx diagnostic

            _ => {
                match self.current_token.token_type {
                    TokenType::Eof => {
                        self.parser_error("Unexpected end of file: expected an expression")
                    }
                    TokenType::Semicolon => self.parser_error("Expected an expression before ';'"),
                    _ => self.parser_error(&format!(
                        "Unexpected token '{}': expected an expression",
                        self.current_token.literal
                    )),
                }
                None
            }
        };

        // ── INFIX ─────────────────────────────────────────────────────────────
        self.parse_infix_chain(left_exp, precedence)
    }

    /// Continues the infix chain starting from an already-parsed left expression.
    /// Used by both parse_expression and the lambda fallback grouped-expr case.
    pub(super) fn parse_infix_chain(
        &mut self,
        mut left_exp: Option<Expression>,
        precedence: Precedence,
    ) -> Option<Expression> {
        // Every iteration appends one level to the left spine of the tree being
        // built. The guard holds all of them until the chain is finished, so
        // the ceiling bounds the tree's depth and not merely the parser's own
        // recursion. See `charge_depth`.
        let mut chain = DepthGuard::empty(&self.depth);
        while self.peek_token.token_type != TokenType::Semicolon
            && precedence < self.peek_precedence()
        {
            let is_infix = match self.peek_token.token_type {
                TokenType::Plus
                | TokenType::Minus
                | TokenType::Slash
                | TokenType::Asterisk
                | TokenType::Percent
                | TokenType::Eq
                | TokenType::NotEq
                | TokenType::Lt
                | TokenType::Gt
                | TokenType::LtEq
                | TokenType::GtEq
                | TokenType::And
                | TokenType::Or
                | TokenType::NullCoalesce
                | TokenType::Question
                | TokenType::LParen
                | TokenType::Dot
                | TokenType::QuestionDot
                | TokenType::LBracket
                | TokenType::Power
                | TokenType::BitAnd
                | TokenType::BitOr
                | TokenType::BitXor
                | TokenType::Shl
                | TokenType::Shr
                | TokenType::KwIs
                | TokenType::Pipe => true,
                _ => false,
            };

            if !is_infix {
                return left_exp;
            }

            self.charge_depth(&mut chain)?;

            self.next_token();

            let operator = self.current_token.literal.clone();
            let current_precedence = self.current_precedence();

            if self.current_token.token_type == TokenType::LParen {
                if let Some(left) = left_exp {
                    let call_open = self.current_token.span;

                    if let Some(args) = self.parse_call_arguments() {
                        left_exp = Some(Expression::Call(CallExpression {
                            function: Box::new(left),
                            arguments: args,
                            span: self.span_to_here(call_open),
                        }));
                    } else {
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::LBracket {
                if let Some(left) = left_exp {
                    // The `[`. Like a call, the extent runs from the bracket rather
                    // than from the indexed expression, which carries no span yet.
                    let index_open = self.current_token.span;
                    self.next_token();
                    if let Some(index) = self.parse_expression(Precedence::Lowest) {
                        if self.peek_token.token_type != TokenType::RBracket {
                            self.parser_error("Expected ']' after array index");
                            return None;
                        }
                        self.next_token();
                        left_exp = Some(Expression::Index(IndexExpression {
                            left: Box::new(left),
                            index: Box::new(index),
                            span: self.span_to_here(index_open),
                        }));
                    } else {
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::Question {
                // Ternary: condition ? then_expr : else_expr
                if let Some(condition) = left_exp {
                    let ternary_open = self.current_token.span;
                    self.next_token(); // first token of then_expr
                    let then_expr = match self.parse_expression(Precedence::Lowest) {
                        Some(e) => e,
                        None => return None,
                    };
                    if self.peek_token.token_type != TokenType::Colon {
                        self.parser_error("Expected ':' in ternary expression after '?'");
                        return None;
                    }
                    self.next_token(); // ':'
                    self.next_token(); // first token of else_expr
                    let else_expr = match self.parse_expression(Precedence::Lowest) {
                        Some(e) => e,
                        None => return None,
                    };
                    left_exp = Some(Expression::Ternary(TernaryExpression {
                        condition: Box::new(condition),
                        then_expr: Box::new(then_expr),
                        else_expr: Box::new(else_expr),
                        span: self.span_to_here(ternary_open),
                    }));
                }
            } else if self.current_token.token_type == TokenType::KwIs {
                // `expr is TypeName` → Infix("is", expr, Identifier("type_name"))
                let op_open = self.current_token.span;
                self.next_token(); // consume type name token (KwInt, KwString, Ident, etc.)
                let type_name = self.current_token.literal.clone();
                let type_name_span = self.current_token.span;
                if let Some(left) = left_exp {
                    left_exp = Some(Expression::Infix(InfixExpression {
                        left: Box::new(left),
                        operator: "is".to_string(),
                        right: Box::new(Expression::Identifier {
                            name: type_name,
                            span: type_name_span,
                        }),
                        span: self.span_to_here(op_open),
                    }));
                }
            } else if self.current_token.token_type == TokenType::Dot
                || self.current_token.token_type == TokenType::QuestionDot
            {
                let is_optional = self.current_token.token_type == TokenType::QuestionDot;
                let dot_open = self.current_token.span;

                // After '.', accept identifiers AND keyword tokens as method names
                // (e.g. tensor.get(), dict.set(), obj.new() should work)
                if !self.peek_token_is_name() {
                    self.parser_error("Expected method name after '.'");
                    return left_exp;
                }
                self.next_token();
                let method = self.current_token.literal.clone();

                let has_parens = self.peek_token.token_type == TokenType::LParen;
                let arguments = if has_parens {
                    self.next_token();
                    self.parse_call_arguments().unwrap_or_default()
                } else {
                    Vec::new()
                };

                if let Some(left) = left_exp {
                    left_exp = Some(Expression::DotCall(DotCallExpression {
                        object: Box::new(left),
                        method,
                        arguments,
                        has_parens,
                        is_optional,
                        span: self.span_to_here(dot_open),
                    }));
                }
            } else if self.current_token.token_type == TokenType::Pipe {
                // |> desugars: left |> fn  →  fn(left)
                let call_open = self.current_token.span;
                self.next_token(); // advance to the function expression
                if let Some(left) = left_exp {
                    if let Some(func) = self.parse_expression(current_precedence) {
                        left_exp = Some(Expression::Call(CallExpression {
                            function: Box::new(func),
                            arguments: vec![left],
                            span: self.span_to_here(call_open),
                        }));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            } else {
                let op_open = self.current_token.span;

                self.next_token();

                // `**` is right-associative (2 ** 3 ** 2 == 2 ** (3 ** 2)), matching
                // math/Python. Parse its right operand one level below Power so a
                // following `**` binds into the right side. All other operators stay
                // left-associative.
                let right_precedence = if current_precedence == Precedence::Power {
                    Precedence::Product
                } else {
                    current_precedence
                };

                if let Some(left) = left_exp {
                    if let Some(right) = self.parse_expression(right_precedence) {
                        left_exp = Some(Expression::Infix(InfixExpression {
                            left: Box::new(left),
                            operator,
                            right: Box::new(right),
                            span: self.span_to_here(op_open),
                        }));
                    } else {
                        return None;
                    }
                } else {
                    return None;
                }
            }
        }

        left_exp
    }

    // ── new expression ────────────────────────────────────────────────────────
    pub(super) fn parse_new_expression(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        // current = 'new'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected class name after 'new'");
            return None;
        }
        self.next_token();
        let class_name = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after class name in 'new'");
            return None;
        }
        self.next_token(); // '('

        // Distinguish interface { field: val } from positional args
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // '{'
            self.next_token(); // first field name or '}'

            let mut fields: Vec<(String, Expression)> = Vec::new();
            while self.current_token.token_type != TokenType::RBrace
                && self.current_token.token_type != TokenType::Eof
            {
                if self.current_token.token_type != TokenType::Ident {
                    self.parser_error("Expected field name in 'new' interface literal");
                    return None;
                }
                let field_name = self.current_token.literal.clone();
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after field name in 'new'");
                    return None;
                }
                self.next_token(); // ':'
                self.next_token(); // value
                let value = self.parse_expression(Precedence::Lowest)?;
                fields.push((field_name, value));

                match self.peek_token.token_type {
                    TokenType::Comma => {
                        self.next_token(); // ','
                        if self.peek_token.token_type == TokenType::RBrace {
                            self.next_token(); // '}'
                            break;
                        }
                        self.next_token(); // next field
                    }
                    TokenType::RBrace => {
                        self.next_token(); // '}'
                        break;
                    }
                    _ => {
                        self.parser_error("Expected ',' or '}}' in interface fields");
                        return None;
                    }
                }
            }
            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after '}}' in 'new'");
                return None;
            }
            self.next_token(); // ')'
            Some(Expression::New(NewExpression {
                class_name,
                span: self.span_to_here(open),
                args: NewArgs::Fields(fields),
            }))
        } else {
            let args = self.parse_call_arguments()?;
            Some(Expression::New(NewExpression {
                class_name,
                span: self.span_to_here(open),
                args: NewArgs::Positional(args),
            }))
        }
    }

    pub(super) fn parse_sizeof_expression(&mut self) -> Option<Expression> {
        use crate::ast::SizeOfTarget;
        if self.peek_token.token_type != TokenType::LParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '(' after 'sizeof'");
            return None;
        }
        self.next_token(); // consume '('
        self.next_token(); // move to the argument

        let type_names = [
            "int", "decimal", "dec", "bool", "string", "null", "void", "any",
        ];
        let target = if matches!(
            self.current_token.token_type,
            TokenType::KwInt
                | TokenType::KwDecimal
                | TokenType::KwDec
                | TokenType::KwBool
                | TokenType::KwString
                | TokenType::KwNull
                | TokenType::KwVoid
                | TokenType::KwAny
        ) {
            let name = self.current_token.literal.clone();
            self.next_token(); // consume type keyword
            SizeOfTarget::Type(name)
        } else if self.current_token.token_type == TokenType::Ident
            && type_names.contains(&self.current_token.literal.as_str())
        {
            let name = self.current_token.literal.clone();
            self.next_token();
            SizeOfTarget::Type(name)
        } else {
            let expr = self.parse_expression(Precedence::Lowest)?;
            SizeOfTarget::Expr(Box::new(expr))
        };

        if self.current_token.token_type != TokenType::RParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected ')' to close sizeof");
            return None;
        }
        Some(Expression::SizeOf(target))
    }

    pub(super) fn parse_call_arguments(&mut self) -> Option<Vec<Expression>> {
        let mut args = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(args);
        }

        self.next_token();

        // Handle spread in first argument position
        if self.current_token.token_type == TokenType::DotDotDot {
            self.next_token();
            let inner = self.parse_expression(Precedence::Lowest)?;
            args.push(Expression::Spread(Box::new(inner)));
        } else {
            args.push(self.parse_expression(Precedence::Lowest)?);
        }

        while self.peek_token.token_type == TokenType::Comma {
            self.next_token();
            self.next_token();
            if self.current_token.token_type == TokenType::DotDotDot {
                self.next_token();
                let inner = self.parse_expression(Precedence::Lowest)?;
                args.push(Expression::Spread(Box::new(inner)));
            } else {
                args.push(self.parse_expression(Precedence::Lowest)?);
            }
        }

        if self.peek_token.token_type != TokenType::RParen {
            return None;
        }
        self.next_token();

        Some(args)
    }
}
