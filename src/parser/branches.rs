//! Choosing a branch: `if`, `switch`, `match`, and `try`.
//!
//! Together because they are the four ways this language selects between
//! alternatives, and because three of them share `parse_inner_block` — the
//! brace-delimited body that is not a `Statement::Block`. `spec/control-flow.md`
//! is the normative contract for all four.
//!
//! `if` and `match` produce an `Expression`, not a `Statement`, and are reached
//! from `parse_expression`; `switch` and `try` are statements. The split is not
//! tidy and is not this module's to tidy: it is the language's shape, and
//! `spec/control-flow.md` records it.
//!
//! Two hazards `spec/control-flow.md` names, both preserved exactly:
//!
//!   * `match` is **not** checked for exhaustiveness. A value matching no arm is
//!     a runtime concern, not a syntax error, and nothing here looks at whether
//!     the arms cover the domain.
//!   * `else if` is parsed by recursion — the `else` branch re-enters
//!     `parse_if_expression` and wraps the result in a synthetic single-statement
//!     block. So a long `else if` chain costs one level of AST depth per link
//!     and is charged against `MAX_PARSE_DEPTH` accordingly.

use super::literals::parse_dec_literal;
use super::{Parser, Precedence};
use crate::ast::*;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_if_expression(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'if'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after 'if' condition");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start 'if' consequence");
            return None;
        }
        self.next_token();

        let consequence = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        let mut alternative = None;

        if self.peek_token.token_type == TokenType::Else {
            self.next_token();

            if self.peek_token.token_type == TokenType::If {
                self.next_token();

                if let Some(if_expr) = self.parse_if_expression() {
                    alternative = Some(BlockStatement {
                        statements: vec![Statement::Expression(if_expr)],
                        span: crate::span::Span::unknown(),
                    });
                }
            } else {
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' or 'if' after 'else'");
                    return None;
                }
                self.next_token();
                alternative = match self.parse_block_statement()? {
                    Statement::Block(b) => Some(b),
                    _ => None,
                };
            }
        }

        Some(Expression::If(IfExpression {
            condition: Box::new(condition),
            consequence,
            alternative,
            span: self.span_to_here(open),
        }))
    }

    // ── switch (expr) { case v1, v2: { body } ... default: { body } } ─────────
    pub(super) fn parse_switch_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        // switch (expr)
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'switch'");
            return None;
        }
        self.next_token(); // '('
        self.next_token(); // first token of expr
        let value = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after switch expression");
            return None;
        }
        self.next_token(); // ')'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after switch(...)");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first token inside

        let mut cases = Vec::new();
        let mut default = None;

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type == TokenType::KwDefault {
                // default: { body }
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after 'default'");
                    return None;
                }
                self.next_token(); // ':'
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' after 'default:'");
                    return None;
                }
                self.next_token(); // '{'
                let body = self.parse_inner_block()?;
                default = Some(body);
            } else if self.current_token.token_type == TokenType::KwCase {
                // case v1, v2, ...: { body }
                let mut values = Vec::new();
                self.next_token(); // first value
                let first = self.parse_expression(Precedence::Lowest)?;
                values.push(first);
                while self.peek_token.token_type == TokenType::Comma {
                    self.next_token(); // ','
                    self.next_token(); // next value
                    let v = self.parse_expression(Precedence::Lowest)?;
                    values.push(v);
                }
                if self.peek_token.token_type != TokenType::Colon {
                    self.parser_error("Expected ':' after case value(s)");
                    return None;
                }
                self.next_token(); // ':'
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error("Expected '{{' after 'case ...:'");
                    return None;
                }
                self.next_token(); // '{'
                let body = self.parse_inner_block()?;
                cases.push(SwitchCase { values, body });
            } else {
                self.parser_error(&format!(
                    "Expected 'case' or 'default' inside switch, got '{}'",
                    self.current_token.literal
                ));
                return None;
            }
            self.next_token(); // move past '}' of the case body
        }

        Some(Statement::Switch(SwitchStatement {
            value,
            cases,
            default,
            span: self.span_to_here(open),
        }))
    }

    /// Called when current_token == KwMatch. Returns Expression::Match.
    pub(super) fn parse_match_expression(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        // Advance past 'match' to the subject expression
        self.next_token();
        let subject = self.parse_expression(Precedence::Lowest)?;
        // Now current = last token of subject, peek = '{'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{' after match subject");
            return None;
        }
        self.next_token(); // current = '{'
        self.next_token(); // current = first token inside match body

        let mut arms = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Parse pattern (possibly OR-ed with '|')
            let pattern = self.parse_match_pattern()?;

            // Optional guard: if expr
            let guard = if self.peek_token.token_type == TokenType::If {
                self.next_token(); // consume 'if'
                self.next_token(); // start of guard expression
                let g = self.parse_expression(Precedence::Lowest)?;
                Some(Box::new(g))
            } else {
                None
            };

            // Expect '=>'
            if self.peek_token.token_type != TokenType::Arrow {
                self.parser_error("Expected '=>' in match arm");
                return None;
            }
            self.next_token(); // current = '=>'
            self.next_token(); // current = first token of body

            // Parse body: block or single expression
            let body = if self.current_token.token_type == TokenType::LBrace {
                self.parse_inner_block()? // current ends on '}'
            } else {
                let expr = self.parse_expression(Precedence::Lowest)?;
                BlockStatement {
                    statements: vec![Statement::Expression(expr)],
                    span: crate::span::Span::unknown(),
                }
            };

            // Optional trailing ','
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // current = ','
            }

            arms.push(MatchArm {
                pattern,
                guard,
                body,
            });

            // Advance to next arm or closing '}'
            if self.peek_token.token_type == TokenType::RBrace {
                self.next_token(); // current = '}'
                break;
            }
            if self.current_token.token_type != TokenType::RBrace {
                self.next_token();
            }
        }

        Some(Expression::Match(Box::new(MatchExpression {
            subject: Box::new(subject),
            arms,
            span: self.span_to_here(open),
        })))
    }

    /// Parse one match pattern (which may be pat | pat | ...).
    pub(super) fn parse_match_pattern(&mut self) -> Option<MatchPattern> {
        let first = self.parse_single_match_pattern()?;
        if self.peek_token.token_type != TokenType::BitOr {
            return Some(first);
        }
        let mut pats = vec![first];
        while self.peek_token.token_type == TokenType::BitOr {
            self.next_token(); // '|'
            self.next_token(); // start of next pattern
            pats.push(self.parse_single_match_pattern()?);
        }
        Some(MatchPattern::Or(pats))
    }

    /// Parse a single non-OR match pattern.
    pub(super) fn parse_single_match_pattern(&mut self) -> Option<MatchPattern> {
        match self.current_token.token_type {
            TokenType::Ident if self.current_token.literal == "_" => Some(MatchPattern::Wildcard),
            TokenType::Ident if self.peek_token.token_type == TokenType::Dot => {
                // Enum.Variant pattern — e.g. Direction.North
                let name = self.current_token.literal.clone();
                // The pattern's own position, captured before the cursor moves
                // off it. Until M2.3.3 this node was built with
                // `Span::point(0, 0)` — no position at all, though one was right
                // here — so a dispatch failure on an enum pattern reported
                // `0:0`. See ROADMAP_STATE.md §5.22.
                let pattern_open = self.current_token.span;
                self.next_token(); // consume '.'
                if !self.peek_token_is_name() {
                    self.parser_error("Expected variant name after '.' in match pattern");
                    return None;
                }
                self.next_token(); // advance to variant name
                let variant = self.current_token.literal.clone();
                let expr = Expression::DotCall(DotCallExpression {
                    object: Box::new(Expression::Identifier {
                        name,
                        span: pattern_open,
                    }),
                    method: variant,
                    arguments: vec![],
                    has_parens: false,
                    is_optional: false,
                    span: self.span_to_here(pattern_open),
                });
                Some(MatchPattern::Literal(expr))
            }
            TokenType::Ident => Some(MatchPattern::Binding(self.current_token.literal.clone())),
            TokenType::Int => {
                let n: i64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Integer(n)))
            }
            TokenType::Minus => {
                // Negative literal: -42
                self.next_token();
                if self.current_token.token_type != TokenType::Int {
                    self.parser_error("Expected integer after '-' in match pattern");
                    return None;
                }
                let n: i64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Integer(-n)))
            }
            TokenType::Decimal => {
                let n: f64 = self.current_token.literal.parse().ok()?;
                Some(MatchPattern::Literal(Expression::Decimal(n)))
            }
            TokenType::Dec => {
                let d = parse_dec_literal(&self.current_token.literal)?;
                Some(MatchPattern::Literal(Expression::Dec(d)))
            }
            TokenType::String => Some(MatchPattern::Literal(Expression::String(
                self.current_token.literal.clone(),
            ))),
            TokenType::RawString => Some(MatchPattern::Literal(Expression::String(
                self.current_token.literal.clone(),
            ))),
            TokenType::True => Some(MatchPattern::Literal(Expression::Boolean(true))),
            TokenType::False => Some(MatchPattern::Literal(Expression::Boolean(false))),
            TokenType::KwNull => Some(MatchPattern::Literal(Expression::Null)),
            _ => {
                self.parser_error(&format!(
                    "Unexpected token '{}' in match pattern",
                    self.current_token.literal
                ));
                None
            }
        }
    }

    // ── try { } catch (e) { } finally { } ────────────────────────────────────
    pub(super) fn parse_try_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        // try { body }
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after 'try'");
            return None;
        }
        self.next_token(); // '{'
        let body = self.parse_inner_block()?;

        let mut catch_var: Option<String> = None;
        let mut catch_body: Option<BlockStatement> = None;
        let mut finally_body: Option<BlockStatement> = None;

        // optional: catch (e) { }
        if self.peek_token.token_type == TokenType::KwCatch {
            self.next_token(); // 'catch'
            if self.peek_token.token_type == TokenType::LParen {
                self.next_token(); // '('
                self.next_token(); // variable name or ')'
                if self.current_token.token_type == TokenType::Ident {
                    catch_var = Some(self.current_token.literal.clone());
                    self.next_token(); // ')'
                }
            }
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' after catch");
                return None;
            }
            self.next_token(); // '{'
            catch_body = Some(self.parse_inner_block()?);
        }

        // optional: finally { }
        if self.peek_token.token_type == TokenType::KwFinally {
            self.next_token(); // 'finally'
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' after 'finally'");
                return None;
            }
            self.next_token(); // '{'
            finally_body = Some(self.parse_inner_block()?);
        }

        if catch_body.is_none() && finally_body.is_none() {
            self.parser_error("'try' must have at least one 'catch' or 'finally' block");
            return None;
        }

        Some(Statement::Try(TryStatement {
            body,
            catch_var,
            catch_body,
            span: self.span_to_here(open),
            finally_body,
        }))
    }

    /// Parse `{ stmts }` — current_token is `{`, leaves current_token on `}`
    pub(super) fn parse_inner_block(&mut self) -> Option<BlockStatement> {
        let open = self.current_token.span;
        self.next_token(); // skip '{'
        let mut statements = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if let Some(s) = self.parse_statement() {
                statements.push(s);
            }
            self.next_token();
        }
        Some(BlockStatement {
            statements,
            span: self.span_to_here(open),
        })
    }
}
