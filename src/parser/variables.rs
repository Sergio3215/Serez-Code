//! Binding a name: `let`, `const`, and the two destructuring shapes.
//!
//! One module because `let` and `const` are the same grammar with one flag, and
//! because both admit the same three left-hand sides — a plain name, an array
//! pattern `[a, _, b, ...rest]`, or a dict pattern `{x, y: renamed}`. A change
//! to what may appear on the left of a binding reaches all of it at once.
//!
//! `spec/variables.md` is the normative contract; `spec/scopes.md` covers what
//! the binding then means, which is emphatically not this file's business.
//!
//! Two things the grammar here does *not* do, both deliberate and both recorded
//! in `spec/syntax.md`:
//!
//!   * a scalar `let` takes no type annotation — `let x: int = 1` is not the
//!     language. A collection does (`let xs [int] = []`, `let d <int,string>`),
//!     which is why the type keywords appear below but never after a `:`.
//!   * `for (let x in xs)` requires the `let`; the pattern parsers here are
//!     shared with that form, which is why they are `pub(super)` rather than
//!     private to this module.

use super::types::is_type_keyword;
use super::{Parser, Precedence};
use crate::ast::*;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_let_statement(&mut self) -> Option<Statement> {
        let is_const = self.current_token.token_type == TokenType::KwConst;

        // Array destructuring: let [a, b, ...rest] = expr;
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // current = '['
            let (names, rest) = self.parse_array_destructure_pattern()?;
            // current is now ']'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after array destructure pattern");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of value
            let value = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::LetDestructureArray(LetDestructureArray {
                names,
                rest,
                value,
                is_const,
            }));
        }

        // Dict destructuring: let {key, key: alias} = expr;
        if self.peek_token.token_type == TokenType::LBrace {
            self.next_token(); // current = '{'
            let fields = self.parse_dict_destructure_pattern()?;
            // current is now '}'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after dict destructure pattern");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of value
            let value = self.parse_expression(Precedence::Lowest)?;
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::LetDestructureDict(LetDestructureDict {
                fields,
                value,
                is_const,
            }));
        }

        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error(&format!(
                "Expected variable name after '{}'",
                if is_const { "const" } else { "let" }
            ));
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();

        // Typed array annotation: let name [type] = [...]
        if self.peek_token.token_type == TokenType::LBracket {
            self.next_token(); // consume '['
            self.next_token(); // move to type keyword
            if !is_type_keyword(&self.current_token.token_type) {
                self.parser_error("Expected type keyword inside '[...]' array annotation");
                return None;
            }
            let element_type = self.parse_type_string()?;
            if self.peek_token.token_type != TokenType::RBracket {
                self.parser_error("Expected ']' after array type annotation");
                return None;
            }
            self.next_token(); // consume ']'
            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after array type annotation");
                return None;
            }
            self.next_token(); // '='
            self.next_token(); // first token of RHS
            let mut value = self.parse_expression(Precedence::Lowest)?;
            match &mut value {
                Expression::ArrayLiteral(arr) => arr.element_type = Some(element_type),
                _ => {
                    self.parser_error(
                        "Expected '[...]' array literal after typed array annotation",
                    );
                    return None;
                }
            }
            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }
            return Some(Statement::Let(LetStatement {
                name,
                value,
                is_const,
            }));
        }

        if self.peek_token.token_type == TokenType::Lt {
            let (key_type, value_type) = self.parse_dict_type_annotation()?;

            if self.peek_token.token_type != TokenType::Assign {
                self.parser_error("Expected '=' after dict type annotation");
                return None;
            }
            self.next_token();
            self.next_token();

            if self.current_token.token_type != TokenType::LParen {
                self.parser_error("Expected '(' to start dict literal");
                return None;
            }

            let value = self.parse_dict_literal(key_type, value_type)?;

            if self.peek_token.token_type == TokenType::Semicolon {
                self.next_token();
            }

            return Some(Statement::Let(LetStatement {
                name,
                value,
                is_const,
            }));
        }

        if self.peek_token.token_type != TokenType::Assign {
            self.parser_error(&format!(
                "Expected '=' after variable name '{}' in {} declaration",
                name,
                if is_const { "const" } else { "let" }
            ));
            return None;
        }
        self.next_token(); // '='
        self.next_token(); // first token of value

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Let(LetStatement {
            name,
            value,
            is_const,
        }))
    }

    // Parse `[a, _, b, ...rest]` — caller must be positioned at `[`.
    // Returns (slots, rest_name). Leaves current_token at `]`.
    pub(super) fn parse_array_destructure_pattern(
        &mut self,
    ) -> Option<(Vec<Option<String>>, Option<String>)> {
        // current = '['
        let mut slots: Vec<Option<String>> = Vec::new();
        let mut rest: Option<String> = None;

        // empty pattern []
        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token(); // current = ']'
            return Some((slots, rest));
        }

        loop {
            self.next_token(); // current = name | _ | ... | ]
            match self.current_token.token_type.clone() {
                TokenType::RBracket => break,
                TokenType::Ident => {
                    let name = self.current_token.literal.clone();
                    slots.push(if name == "_" { None } else { Some(name) });
                }
                TokenType::DotDotDot => {
                    // rest element
                    if self.peek_token.token_type != TokenType::Ident {
                        self.parser_error("Expected identifier after '...' in destructure");
                        return None;
                    }
                    self.next_token();
                    rest = Some(self.current_token.literal.clone());
                    // must be followed by ']'
                    if self.peek_token.token_type != TokenType::RBracket {
                        self.parser_error("Rest element must be last in array destructure");
                        return None;
                    }
                    self.next_token(); // current = ']'
                    break;
                }
                _ => {
                    self.parser_error("Expected identifier or '...' in array destructure pattern");
                    return None;
                }
            }
            // after a slot: expect ',' or ']'
            match self.peek_token.token_type {
                TokenType::Comma => {
                    self.next_token();
                } // consume ','
                TokenType::RBracket => {
                    self.next_token();
                    break;
                } // consume ']'
                _ => {
                    self.parser_error("Expected ',' or ']' in array destructure pattern");
                    return None;
                }
            }
        }
        Some((slots, rest))
    }

    // Parse `{key, key: alias}` — caller must be positioned at `{`.
    // Returns Vec<(key, local_alias)>. Leaves current_token at `}`.
    pub(super) fn parse_dict_destructure_pattern(
        &mut self,
    ) -> Option<Vec<(String, Option<String>)>> {
        // current = '{'
        let mut fields: Vec<(String, Option<String>)> = Vec::new();

        if self.peek_token.token_type == TokenType::RBrace {
            self.next_token(); // current = '}'
            return Some(fields);
        }

        loop {
            self.next_token(); // current = key name
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected property name in dict destructure pattern");
                return None;
            }
            let key = self.current_token.literal.clone();

            // optional rename: {key: alias}
            let alias = if self.peek_token.token_type == TokenType::Colon {
                self.next_token(); // consume ':'
                if self.peek_token.token_type != TokenType::Ident {
                    self.parser_error("Expected identifier after ':' in dict destructure");
                    return None;
                }
                self.next_token();
                Some(self.current_token.literal.clone())
            } else {
                None
            };
            fields.push((key, alias));

            match self.peek_token.token_type {
                TokenType::Comma => {
                    self.next_token();
                }
                TokenType::RBrace => {
                    self.next_token();
                    break;
                }
                _ => {
                    self.parser_error("Expected ',' or '}}' in dict destructure pattern");
                    return None;
                }
            }
        }
        Some(fields)
    }
}
