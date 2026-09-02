//! Loops, and the labels that name them.
//!
//! `while`, `do-while`, the C-style `for`, `for-in`, and the `label:` prefix
//! that lets `break`/`continue` name which loop they mean. `spec/control-flow.md`
//! is the normative contract.
//!
//! Labels live here rather than with the other statement forms because
//! `parse_labeled_statement` exists only to introduce a loop: it reads the name,
//! then requires `while` or `for` immediately after. A label in front of
//! anything else is a syntax error, which is why the whole of it is loop
//! grammar despite looking like a general statement prefix.
//!
//! `parse_for_inner` is the large one, and it is large because one keyword opens
//! three different statements: `for (init; cond; step)`, `for (let x in xs)`,
//! and the same two with a label already consumed. It decides between them by
//! lookahead rather than by backtracking, so the shapes are interleaved in one
//! function. Splitting that is a change to how the decision is made, not a move,
//! so it is left exactly as it was.
//!
//! `spec/syntax.md` records that `for-in` requires the `let` — `for (x in xs)`
//! is not the language — and that the pattern parsers it shares with `let` live
//! in `variables.rs`.

use super::{Parser, Precedence};
use crate::ast::*;
use crate::span::Span;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_for_inner(&mut self, label: Option<String>) -> Option<Statement> {
        // The `for` keyword. Both the classic and the for-in shapes end up
        // here, and both report from it.
        let open = self.current_token.span;
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'for'");
            return None;
        }
        self.next_token(); // current = '('
        self.next_token(); // current = 'let'

        if self.current_token.token_type != TokenType::Let {
            self.parser_error("Expected 'let' as for-loop initializer");
            return None;
        }

        // The `let` inside `for (`. Captured here because by the time the
        // initializer is built the cursor has moved past the `;` onto the
        // condition, and an extent taken then would swallow it.
        let init_open = self.current_token.span;

        // ── ForEach with array destructuring: for (let [a, b] in ...) ─────────
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // current = '['
            let (slots, rest) = self.parse_array_destructure_pattern()?;
            // current is now ']'
            if self.peek_token.token_type != TokenType::KwIn {
                self.parser_error("Expected 'in' after destructure pattern in for");
                return None;
            }
            self.next_token(); // current = 'in'
            self.next_token(); // current = first token of iterable
            let iterable = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after for-in iterable");
                return None;
            }
            self.next_token(); // current = ')'
            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start for-in body");
                return None;
            }
            self.next_token();
            let body = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };
            return Some(Statement::ForEach(ForEachStatement {
                var: ForEachVar::Array(slots, rest),
                iterable,
                body,
                label: label.clone(),
                span: self.span_to_here(open),
            }));
        }

        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected identifier after 'let' in for");
            return None;
        }
        self.next_token(); // current = var_name
        let var_name = self.current_token.literal.clone();

        // ── ForEach: for (let x in iterable) { body } ────────────────────────
        if self.peek_token.token_type == TokenType::KwIn {
            self.next_token(); // current = 'in'
            self.next_token(); // current = first token of iterable
            let iterable = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::RParen {
                self.parser_error("Expected ')' after for-in iterable");
                return None;
            }
            self.next_token(); // current = ')'

            if self.peek_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start for-in body");
                return None;
            }
            self.next_token(); // current = '{'

            let body = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };

            return Some(Statement::ForEach(ForEachStatement {
                var: ForEachVar::Name(var_name),
                iterable,
                body,
                label: label.clone(),
                span: self.span_to_here(open),
            }));
        }

        // ── Classic for: for (let i = 0; i < n; i = i + 1) { body } ─────────
        if self.peek_token.token_type != TokenType::Assign {
            self.parser_error("Expected '=' or 'in' after variable name in for");
            return None;
        }
        self.next_token(); // current = '='
        self.next_token(); // current = first token of init value
        let init_value = self.parse_expression(Precedence::Lowest)?;
        // Cursor is on the last token of the initializer, before the `;`.
        let init_span = self.span_to_here(init_open);

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token(); // current = ';'
        } else if self.current_token.token_type != TokenType::Semicolon {
            self.parser_error("Expected ';' after for-loop initializer");
            return None;
        }
        self.next_token(); // current = first token of condition

        let init = LetStatement {
            name: var_name,
            value: init_value,
            is_const: false,
            span: init_span,
        };

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Semicolon {
            self.parser_error("Expected ';' after for-loop condition");
            return None;
        }
        self.next_token();
        self.next_token();

        if self.current_token.token_type != TokenType::Ident {
            self.parser_error("Expected assignment as for-loop update");
            return None;
        }
        let update = match self.peek_token.token_type {
            TokenType::Assign => match self.parse_assign_statement()? {
                Statement::Assign(a) => a,
                _ => return None,
            },
            TokenType::PlusPlus | TokenType::MinusMinus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                let op = if self.peek_token.token_type == TokenType::PlusPlus {
                    "+"
                } else {
                    "-"
                };
                self.next_token(); // consume ++ / --
                AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier {
                            name,
                            span: Span::point(line, col),
                        }),
                        operator: op.to_string(),
                        right: Box::new(Expression::Integer(1)),
                        span: Span::point(line, col),
                    }),
                    span: Span::point(line, col),
                }
            }
            ref tt if self.is_compound_assign(&tt.clone()) => {
                match self.parse_compound_assign_statement()? {
                    Statement::Assign(a) => a,
                    _ => return None,
                }
            }
            _ => {
                self.parser_error("Expected assignment as for-loop update");
                return None;
            }
        };

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after for-loop update");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start for-loop body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::For(ForStatement {
            init,
            condition,
            update,
            body,
            label,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn parse_for_statement(&mut self) -> Option<Statement> {
        self.parse_for_inner(None)
    }

    pub(super) fn parse_for_statement_with_label(
        &mut self,
        label: Option<String>,
    ) -> Option<Statement> {
        self.parse_for_inner(label)
    }

    pub(super) fn parse_while_statement(&mut self) -> Option<Statement> {
        self.parse_while_statement_with_label(None)
    }

    pub(super) fn parse_while_statement_with_label(
        &mut self,
        label: Option<String>,
    ) -> Option<Statement> {
        let open = self.current_token.span;
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'while'");
            return None;
        }
        self.next_token();
        self.next_token();

        let condition = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after condition in 'while'");
            return None;
        }
        self.next_token();

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' to start 'while' body");
            return None;
        }
        self.next_token();

        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };

        Some(Statement::While(WhileStatement {
            condition,
            body,
            label,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn parse_do_while_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        // current = 'do'
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after 'do'");
            return None;
        }
        self.next_token(); // current = '{'
        let body = match self.parse_block_statement()? {
            Statement::Block(b) => b,
            _ => return None,
        };
        // current = '}', peek = 'while'
        if self.peek_token.token_type != TokenType::While {
            self.parser_error("Expected 'while' after 'do' body");
            return None;
        }
        self.next_token(); // current = 'while'
        if self.peek_token.token_type != TokenType::LParen {
            self.parser_error("Expected '(' after 'while' in do-while");
            return None;
        }
        self.next_token(); // current = '('
        self.next_token(); // current = first token of condition
        let condition = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RParen {
            self.parser_error("Expected ')' after condition in do-while");
            return None;
        }
        self.next_token(); // current = ')'
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token(); // consume ';'
        }
        Some(Statement::DoWhile(WhileStatement {
            condition,
            body,
            label: None,
            span: self.span_to_here(open),
        }))
    }

    // ── labeled loop: label: while(...) { } ──────────────────────────────────
    pub(super) fn parse_labeled_statement(&mut self) -> Option<Statement> {
        // current = Ident (label), peek = ':'
        let label = self.current_token.literal.clone();
        self.next_token(); // ':'
        self.next_token(); // while / for / ...

        match self.current_token.token_type {
            TokenType::While => self.parse_while_statement_with_label(Some(label)),
            TokenType::For => self.parse_for_statement_with_label(Some(label)),
            _ => {
                // Fall back: not a labeled loop, re-interpret as assign
                self.parser_error(&format!(
                    "Expected 'while' or 'for' after label '{}'",
                    label
                ));
                None
            }
        }
    }
}
