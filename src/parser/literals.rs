//! Writing a value down: arrays, dicts, entries, brace forms, and the two
//! literal lexemes that need real parsing rather than a token.
//!
//! `spec/values.md` and `spec/dicts.md` are the normative contracts.
//!
//! The brace forms are the awkward part of this file and the reason it exists
//! as one. A `{` in expression position can begin three different things —
//! an entry literal, an object patch, or a dict body — and the language
//! deliberately does **not** have a JSON-style object literal, which
//! `spec/syntax.md` records under "Known gaps" along with the rule that a dict
//! literal needs a dict context. `parse_brace_expression` is the single place
//! that decides between them, so the three forms it can produce belong beside
//! it rather than scattered.
//!
//! Two lexeme parsers live here rather than in the lexer, because both need to
//! build an `Expression` and the lexer only produces tokens:
//!
//!   * `parse_dec_literal` turns `12.50` or `1e-7` into an exact decimal.
//!   * `parse_interpolated_string` re-parses the inside of `"a {b + c} d"`,
//!     running a nested parser over each `{…}`. It is also the tenth §5.17 site:
//!     an unclosed `{` prints by hand and cannot reach `take_errors()`, because
//!     it is a free function with no access to the parser's state at all.

use super::{Parser, Precedence};
use crate::ast::*;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_array_literal(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        let mut elements = Vec::new();

        if self.peek_token.token_type == TokenType::RBracket {
            self.next_token();
            return Some(Expression::ArrayLiteral(ArrayLiteral {
                element_type: None,
                elements,
                span: self.span_to_here(open),
            }));
        }

        self.next_token();

        loop {
            let expr = if self.current_token.token_type == TokenType::DotDotDot {
                let spread_open = self.current_token.span;
                self.next_token();
                let inner = self.parse_expression(Precedence::Lowest)?;
                Some(Expression::Spread {
                    value: Box::new(inner),
                    span: self.span_to_here(spread_open),
                })
            } else {
                self.parse_expression(Precedence::Lowest)
            };

            if let Some(e) = expr {
                elements.push(e);
            }

            if self.peek_token.token_type == TokenType::RBracket {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Missing closing bracket ']' or comma ',' in array");
                return None;
            }

            self.next_token();
            self.next_token();
        }

        Some(Expression::ArrayLiteral(ArrayLiteral {
            element_type: None,
            elements,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn parse_dict_literal(
        &mut self,
        key_type: String,
        value_type: String,
    ) -> Option<Expression> {
        let open = self.current_token.span;
        let mut entries = Vec::new();

        if self.peek_token.token_type == TokenType::RParen {
            self.next_token();
            return Some(Expression::DictLiteral(DictLiteral {
                key_type,
                value_type,
                entries,
                span: self.span_to_here(open),
            }));
        }

        self.next_token(); // first '{'

        loop {
            if self.current_token.token_type != TokenType::LBrace {
                self.parser_error("Expected '{{' to start dict entry");
                return None;
            }
            self.next_token();

            let key = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Expected ',' between key and value in dict entry");
                return None;
            }
            self.next_token();
            self.next_token();

            let value = self.parse_expression(Precedence::Lowest)?;

            if self.peek_token.token_type != TokenType::RBrace {
                self.parser_error("Expected '}}' to close dict entry");
                return None;
            }
            self.next_token(); // '}'

            entries.push((key, value));

            if self.peek_token.token_type == TokenType::RParen {
                self.next_token();
                break;
            }

            if self.peek_token.token_type != TokenType::Comma {
                self.parser_error("Expected ',' or ')' after dict entry");
                return None;
            }
            self.next_token(); // ','
            self.next_token(); // next '{'
        }

        Some(Expression::DictLiteral(DictLiteral {
            key_type,
            value_type,
            entries,
            span: self.span_to_here(open),
        }))
    }

    pub(super) fn parse_entry_literal(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        self.next_token();

        let key = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value in entry literal");
            return None;
        }
        self.next_token();
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::RBrace {
            self.parser_error("Expected '}}' to close entry literal");
            return None;
        }
        self.next_token();

        Some(Expression::EntryLiteral {
            key: Box::new(key),
            value: Box::new(value),
            span: self.span_to_here(open),
        })
    }

    // current = first ident of key (already consumed '{'); continue parsing full key expression
    pub(super) fn parse_entry_literal_from_key(
        &mut self,
        key_start: Expression,
    ) -> Option<Expression> {
        let open = self.current_token.span;
        // The key might be more than just the ident (e.g. nombres[i])
        let key = self.parse_infix_chain(Some(key_start), Precedence::Lowest)?;

        if self.peek_token.token_type != TokenType::Comma {
            self.parser_error("Expected ',' between key and value in entry literal");
            return None;
        }
        self.next_token(); // ','
        self.next_token(); // value
        let value = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type != TokenType::RBrace {
            self.parser_error("Expected '}}' to close entry literal");
            return None;
        }
        self.next_token(); // '}'
        Some(Expression::EntryLiteral {
            key: Box::new(key),
            value: Box::new(value),
            span: self.span_to_here(open),
        })
    }

    // ── { ... } disambiguation ────────────────────────────────────────────────
    // When '{' appears in expression context:
    //   - If next token is Ident and next-next is ':' → ObjectPatch { field: val, ... }
    //   - Otherwise → EntryLiteral {key, value} (for dict method args)
    pub(super) fn parse_brace_expression(&mut self) -> Option<Expression> {
        if self.peek_token.token_type == TokenType::Ident {
            // Consume '{', now current = Ident
            self.next_token();
            if self.peek_token.token_type == TokenType::Colon {
                // ObjectPatch: { field: val, ... }
                return self.parse_object_patch_from_ident();
            } else {
                // Entry literal: { ident, value }
                let key = Expression::Identifier {
                    name: self.current_token.literal.clone(),
                    span: self.current_token.span,
                };
                return self.parse_entry_literal_from_key(key);
            }
        }
        self.parse_entry_literal()
    }

    // current = first field name (already consumed '{' and Ident)
    pub(super) fn parse_object_patch_from_ident(&mut self) -> Option<Expression> {
        let open = self.current_token.span;
        let mut fields = Vec::new();
        loop {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected field name in object literal");
                return None;
            }
            let name = self.current_token.literal.clone();
            if self.peek_token.token_type != TokenType::Colon {
                self.parser_error("Expected ':' after field name in object literal");
                return None;
            }
            self.next_token(); // ':'
            self.next_token(); // value
            let value = self.parse_expression(Precedence::Lowest)?;
            fields.push((name, value));

            match self.peek_token.token_type {
                TokenType::Comma => {
                    self.next_token(); // ','
                    if self.peek_token.token_type == TokenType::RBrace {
                        self.next_token(); // '}'
                        break;
                    }
                    self.next_token(); // next field name
                }
                TokenType::RBrace => {
                    self.next_token(); // '}'
                    break;
                }
                _ => {
                    self.parser_error("Expected ',' or '}}' in object literal");
                    return None;
                }
            }
        }
        Some(Expression::ObjectPatch {
            fields,
            span: self.span_to_here(open),
        })
    }
}

/// Parse a `dec` literal lexeme (the `m` suffix is already stripped). Handles
/// both plain (`12.50`) and scientific (`1e-7`) forms via rust_decimal.
pub(super) fn parse_dec_literal(lit: &str) -> Option<rust_decimal::Decimal> {
    if lit.contains('e') || lit.contains('E') {
        rust_decimal::Decimal::from_scientific(lit).ok()
    } else {
        lit.parse::<rust_decimal::Decimal>().ok()
    }
}

pub(super) fn parse_interpolated_string(
    raw: &str,
    source_name: Option<&str>,
) -> Option<Expression> {
    use crate::lexer::Lexer;
    let mut parts: Vec<StringPart> = Vec::new();
    let mut rest = raw;

    while let Some(open) = rest.find('{') {
        if open > 0 {
            // \x01 is the sentinel for \{ (escaped brace) — restore it as a literal {
            parts.push(StringPart::Literal(rest[..open].replace('\x01', "{")));
        }
        let after_open = &rest[open + 1..];
        // Find the matching '}', skipping nested braces and inner strings
        let close = {
            let mut depth: usize = 0;
            let mut in_str = false;
            let mut found = None;
            for (i, c) in after_open.char_indices() {
                if in_str {
                    if c == '"' {
                        in_str = false;
                    }
                } else {
                    match c {
                        '"' => in_str = true,
                        '{' => depth += 1,
                        '}' if depth > 0 => depth -= 1,
                        '}' => {
                            found = Some(i);
                            break;
                        }
                        _ => {}
                    }
                }
            }
            match found {
                Some(c) => c,
                None => {
                    eprintln!("❌ PARSER ERROR: Unclosed '{{' in string interpolation");
                    return None;
                }
            }
        };
        let expr_src = after_open[..close].trim();
        if !expr_src.is_empty() {
            let lexer = Lexer::new(expr_src.to_string());
            let mut sub = Parser::new(lexer);
            if let Some(n) = source_name {
                sub.set_source_name(n);
            }
            let expr = sub.parse_expression(Precedence::Lowest)?;
            parts.push(StringPart::Expr(Box::new(expr)));
        }
        rest = &after_open[close + 1..];
    }

    if !rest.is_empty() {
        parts.push(StringPart::Literal(rest.replace('\x01', "{")));
    }

    if parts.len() == 1 {
        if let StringPart::Literal(ref s) = parts[0] {
            return Some(Expression::String {
                value: s.clone(),
                // A free function with no cursor: the interpolation collapsed to
                // one literal, and its position is the string it came from.
                span: crate::span::Span::unknown(),
            });
        }
    }

    Some(Expression::InterpolatedString {
        parts,
        // A free function with no cursor: the pieces carry their own positions,
        // and the string that produced them is the caller's to know.
        span: crate::span::Span::unknown(),
    })
}
