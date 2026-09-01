//! How a callable is written: declared, given parameters, or spelled inline.
//!
//! Five forms, one subject. `fn name(...)` and `fn* name(...)` declare one;
//! `native fn` declares one whose body lives outside the language; `x => expr`
//! and `(a, b) => { ... }` spell one where it is used. They share
//! `parse_function_parameters`, which is why they share a file: a change to how
//! a parameter list is written has to reach all of them at once, and that is the
//! reason this module exists rather than the line count.
//!
//! `spec/functions.md` is the normative contract.
//!
//! Two limits of the current grammar, both recorded in `spec/syntax.md` under
//! "Known gaps" and both preserved here unchanged:
//!
//!   * a lambda parameter takes no type and no default — the shorthand and the
//!     declaration form are not the same grammar;
//!   * a required parameter after a defaulted one is rejected, which is a
//!     coded diagnostic (`tests/err_parse_required_after_default.sz`).
//!
//! `parse_native_declaration` is also one of the sites in §5.17: three of its
//! failures print by hand instead of going through `parser_error`, so they carry
//! no code and never reach `take_errors()`. Moved as it was; M3 owns the fix.

use super::types::is_type_keyword;
use super::{Parser, Precedence};
use crate::ast::*;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_function_statement(&mut self) -> Option<Statement> {
        // fn* generator syntax: consume the '*'
        let is_generator = self.peek_token.token_type == TokenType::Asterisk;
        if is_generator {
            self.next_token();
        } // consume '*'

        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = self.parse_type_string();
        } else if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // '['
            self.next_token(); // type keyword
            if !is_type_keyword(&self.current_token.token_type) {
                self.parser_error("Expected type keyword inside '[...]' return type");
                return None;
            }
            let elem_type = self.parse_type_string()?;
            if self.peek_token.token_type != TokenType::RBracket {
                self.parser_error("Expected ']' after return type annotation");
                return None;
            }
            self.next_token(); // ']'
            return_type = Some(format!("[{}]", elem_type));
        }

        if self.peek_token.token_type == TokenType::Ident {
            self.next_token();
            // parse_type_string also consumes an optional '?' (for nullable class types)
            let first = self.parse_type_string().unwrap_or_default();

            // Disambiguate: fn ClassName[?] funcName(...) vs fn funcName(...)
            let name = if self.peek_token.token_type == TokenType::Ident {
                return_type = Some(first);
                self.next_token();
                self.current_token.literal.clone()
            } else {
                first
            };

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

            let function = FunctionLiteral {
                return_type,
                parameters,
                body,
                is_generator,
            };

            Some(Statement::FunctionDeclaration(FunctionDeclaration {
                name,
                function,
            }))
        } else {
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

            let function = FunctionLiteral {
                return_type,
                parameters,
                body,
                is_generator,
            };

            Some(Statement::Expression(Expression::FunctionLiteral(function)))
        }
    }

    pub(super) fn parse_function_parameters(&mut self) -> Option<Vec<Parameter>> {
        let mut parameters = Vec::new();
        let mut saw_default = false;

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(parameters);
        }

        self.next_token();

        loop {
            let mut type_name = None;

            if self.current_token.token_type == TokenType::LBracket {
                // [type] array parameter annotation
                self.next_token(); // move to type keyword
                if !is_type_keyword(&self.current_token.token_type) {
                    self.parser_error("Expected type keyword inside '[...]' parameter annotation");
                    return None;
                }
                let elem_type = self.parse_type_string()?;
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array parameter type");
                    return None;
                }
                self.next_token(); // consume ']'
                type_name = Some(format!("[{}]", elem_type));
                self.next_token(); // advance to param name
            } else if is_type_keyword(&self.current_token.token_type) {
                type_name = self.parse_type_string();
                self.next_token();
            } else if self.current_token.token_type == TokenType::Ident
                && (self.peek_token.token_type == TokenType::Ident
                    || self.peek_token.token_type == TokenType::Question)
            {
                // Class type annotation (possibly nullable): fn void f(ClassName[?] param)
                type_name = self.parse_type_string();
                self.next_token();
            }

            // Check for rest parameter `...name`
            let is_rest = if self.current_token.token_type == TokenType::DotDotDot {
                self.next_token(); // advance to param name
                true
            } else {
                false
            };

            let name = if self.current_token.token_type == TokenType::Ident {
                self.current_token.literal.clone()
            } else {
                return None;
            };

            // Optional default value: param = expr
            let default_value = if !is_rest && self.peek_token.token_type == TokenType::Assign {
                self.next_token(); // '='
                self.next_token(); // first token of default expr
                Some(self.parse_expression(Precedence::Lowest)?)
            } else {
                None
            };

            if default_value.is_some() {
                saw_default = true;
            } else if saw_default && !is_rest {
                self.parser_error("Required parameter cannot follow a default parameter");
                return None;
            }

            parameters.push(Parameter {
                name,
                type_name,
                is_rest,
                default_value,
            });

            if is_rest {
                if self.peek_token.token_type != TokenType::RParen {
                    self.parser_error("Rest parameter must be last");
                    return None;
                }
                break;
            }

            if self.peek_token.token_type == TokenType::Comma {
                self.next_token();
                self.next_token();
            } else {
                break;
            }
        }

        if self.peek_token.token_type != TokenType::RParen {
            return None;
        }
        self.next_token();

        Some(parameters)
    }

    pub(super) fn parse_native_declaration(&mut self) -> Option<Statement> {
        use crate::ast::NativeFnDeclaration;
        // native fn [return_type] name(params);
        if self.peek_token.token_type != TokenType::Function {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected 'fn' after 'native'");
            return None;
        }
        self.next_token(); // consume 'fn'

        // optional return type
        let mut return_type = None;
        if is_type_keyword(&self.peek_token.token_type) {
            self.next_token();
            return_type = self.parse_type_string();
        }

        // Disambiguate: native fn ClassName funcName  vs  native fn funcName
        if self.peek_token.token_type != TokenType::Ident {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected function name after 'native fn'");
            return None;
        }
        self.next_token();
        let first = self.current_token.literal.clone();
        let name = if self.peek_token.token_type == TokenType::Ident {
            return_type = Some(first);
            self.next_token();
            self.current_token.literal.clone()
        } else {
            first
        };

        if self.peek_token.token_type != TokenType::LParen {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '(' after native function name");
            return None;
        }
        self.next_token();
        let parameters = self.parse_function_parameters()?;

        // allow trailing {} (empty body) or just ;
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token();
            self.next_token(); // skip '{'
            // consume until '}'
            while self.current_token.token_type != TokenType::RBrace
                && self.current_token.token_type != TokenType::Eof
            {
                self.next_token();
            }
        } else if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::NativeDeclaration(NativeFnDeclaration {
            name,
            return_type,
            parameters,
        }))
    }

    pub(super) fn parse_arrow_function(&mut self) -> Option<Expression> {
        let return_type = self.parse_type_string();

        if self.peek_token.token_type != TokenType::LParen {
            return None;
        }
        self.next_token();

        let parameters = self.parse_function_parameters()?;

        if self.peek_token.token_type != TokenType::Arrow {
            return None;
        }
        self.next_token();

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
        }))
    }

    pub(super) fn parse_lambda_body(&mut self) -> Option<LambdaBody> {
        // current = '=>'
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // '{'
            let block = match self.parse_block_statement()? {
                Statement::Block(b) => b,
                _ => return None,
            };
            Some(LambdaBody::Block(block))
        } else {
            self.next_token(); // first token of expression
            let expr = self.parse_expression(Precedence::Lowest)?;
            Some(LambdaBody::Expr(Box::new(expr)))
        }
    }
}
