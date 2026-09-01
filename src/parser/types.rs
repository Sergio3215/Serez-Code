//! Type *syntax*: recognising a type where one may appear, and reading it.
//!
//! The boundary this file draws is the one `spec/types.md` already draws, and
//! it is worth stating because it is easy to erode: **parsing a type belongs to
//! the parser; deciding whether two types are compatible does not.** Nothing
//! here asks what a type means, whether a value satisfies it, or whether one
//! type may stand in for another. Those questions belong to the type checker
//! and, today, mostly to the runtime — `spec/types.md` records how partial the
//! static side currently is.
//!
//! So a type reaches the AST as a `String`, exactly as written. That is a
//! deliberate limit of the current design rather than an oversight: it means
//! the parser has no opinion to be wrong about, and it is why `parse_type_string`
//! is nine lines. When M5 gives types a real representation, this file is where
//! the syntax half of that change lands.
//!
//! Called from seven places — native declarations, `let`, function statements
//! and their parameters, arrow functions, interfaces and classes — which is why
//! it is a shared module rather than living with any one of them.

use super::Parser;
use crate::token::TokenType;

/// Is this token one of the seven built-in type keywords?
///
/// Deliberately not "is this a type name": a class name is also a legal
/// annotation, and the caller that knows it is looking at one says so. This
/// answers the narrower question the grammar actually asks at a `<` or after a
/// `:`.
pub(super) fn is_type_keyword(token_type: &TokenType) -> bool {
    matches!(
        token_type,
        TokenType::KwVoid
            | TokenType::KwInt
            | TokenType::KwDecimal
            | TokenType::KwDec
            | TokenType::KwString
            | TokenType::KwBool
            | TokenType::KwAny
    )
}

impl Parser {
    // Reads current token as a base type and optionally appends '?' if peek is Question.
    // Assumes caller already verified current is a type keyword.
    pub(super) fn parse_type_string(&mut self) -> Option<String> {
        let base = self.current_token.literal.clone();
        if self.peek_token.token_type == TokenType::Question {
            self.next_token();
            Some(format!("{}?", base))
        } else {
            Some(base)
        }
    }

    pub(super) fn parse_dict_type_annotation(&mut self) -> Option<(String, String)> {
        self.next_token(); // '<'
        self.next_token(); // key_type

        if !is_type_keyword(&self.current_token.token_type) {
            self.parser_error(&format!(
                "Expected type keyword for dict key type, got '{}'",
                self.current_token.literal
            ));
            return None;
        }
        let key_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value types in dict annotation");
            return None;
        }
        self.next_token(); // ','
        self.next_token(); // value_type

        if !is_type_keyword(&self.current_token.token_type) {
            self.parser_error(&format!(
                "Expected type keyword for dict value type, got '{}'",
                self.current_token.literal
            ));
            return None;
        }
        let value_type = self.current_token.literal.clone();

        if self.peek_token.token_type != TokenType::Gt {
            self.parser_error("Expected '>' to close dict type annotation");
            return None;
        }
        self.next_token(); // '>'

        Some((key_type, value_type))
    }
}
