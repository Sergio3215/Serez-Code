//! Named types: `class`, `interface`, `enum`, and the modifiers in front of
//! them.
//!
//! These five are one module for a reason the call graph makes plain rather
//! than a taxonomic one. `parse_visibility_statement` sees `public`/`private`
//! and dispatches to **both** a class and an interface;
//! `parse_abstract_or_sealed_class` sees `abstract`/`sealed`, optionally steps
//! over a visibility keyword, and lands on a class. The modifier prefixes are
//! not owned by either declaration — they route between them — so extracting
//! classes and interfaces separately would have stranded the two dispatchers
//! between the files, or duplicated them.
//!
//! `enum` joins them because it is the third way this language introduces a
//! nominal type by name, and because `is_reserved_name` guards all three
//! against shadowing a built-in namespace (`Task`, `Gui`, `Dec`, …).
//!
//! `spec/classes.md` is the normative contract for the class model, including
//! two compatibility caveats it records and this file must not quietly change:
//! construction enforces parameter types, and private access is compared
//! against the runtime receiver's class rather than the declaring one — so a
//! subclass can reach a parent's private members.
//!
//! `parse_class_declaration` and `parse_visibility_statement` are two of the
//! §5.17 sites: a member without an explicit visibility keyword, and a
//! modifier followed by neither `class` nor `interface`, both print by hand and
//! never reach `take_errors()`. Moved as they were.

use super::types::is_type_keyword;
use super::{Parser, Precedence};
use crate::ast::*;
use crate::span::Span;
use crate::token::TokenType;

impl Parser {
    /// Is this name one the runtime already owns as a namespace?
    ///
    /// **This is a semantic check, and it is in the wrong layer.** The names are
    /// not keywords — the lexer returns them as `Ident`, and `class Task {}` is
    /// perfectly well-formed *syntax*. It is rejected because the name collides
    /// with the runtime's namespace table, which is a fact about what the
    /// program means rather than about the shape of its source. M1's Definition
    /// of Done asks for no semantic validation in the parser, and this is the
    /// one exception; it is preserved rather than relocated because moving it
    /// changes *when* the error is reported. M4, the semantic layer, is its
    /// proper home.
    ///
    /// It is also **incomplete and inconsistent**. The evaluator exposes twenty
    /// namespaces and seven are listed here. Measured against the 10.0.0 binary:
    /// `class Task {}` and `class Gui {}` are rejected, while `class Math {}`,
    /// `class File {}`, `class Socket {}` and `class Crypto {}` are accepted —
    /// and a program may then define `class Math`, call `new Math()`, and still
    /// call `Math.floor(3.7)`, both resolving correctly. So the guard is not
    /// preventing a collision the language cannot survive, and which seven names
    /// it covers looks accidental. See `docs/maturity/ROADMAP_STATE.md` §5.20.
    fn is_reserved_name(&self, name: &str) -> bool {
        matches!(
            name,
            "Task" | "Time" | "DateTime" | "System" | "Gui" | "Dec" | "Media"
        )
    }

    // ── Class declaration ─────────────────────────────────────────────────────
    pub(super) fn parse_class_declaration(
        &mut self,
        is_public: bool,
        is_abstract: bool,
        is_sealed: bool,
    ) -> Option<Statement> {
        // current = 'class'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected class name after 'class'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!(
                "'{}' is a reserved system namespace and cannot be used as a class name",
                name
            ));
            return None;
        }

        // Optional inheritance: class Child : Parent
        let parent = if self.peek_token.token_type == TokenType::Colon {
            self.next_token(); // ':'
            if self.peek_token.token_type != TokenType::Ident {
                self.parser_error("Expected parent class name after ':'");
                return None;
            }
            self.next_token();
            Some(self.current_token.literal.clone())
        } else {
            None
        };

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after class name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first member or '}'

        let mut constructor: Option<ClassConstructor> = None;
        let mut methods: Vec<ClassMethod> = Vec::new();
        let mut fields: Vec<ClassField> = Vec::new();

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Check for abstract method prefix
            let is_member_abstract = if self.current_token.token_type == TokenType::KwAbstract {
                self.next_token(); // after 'abstract'
                true
            } else {
                false
            };

            // visibility modifier
            let is_member_public = match self.current_token.token_type {
                TokenType::KwPublic => true,
                TokenType::KwPrivate => false,
                _ => {
                    // If we hit abstract directly after visibility etc.
                    // Or if it's a class field (Ident: type = value;)
                    // Try to parse as a class field
                    if self.current_token.token_type == TokenType::Ident {
                        let field_name = self.current_token.literal.clone();
                        let field_line = self.current_token.line;
                        let field_col = self.current_token.column;

                        if self.peek_token.token_type == TokenType::Colon {
                            // field: type [= expr];
                            self.next_token(); // ':'
                            self.next_token(); // type
                            let type_annotation = if is_type_keyword(&self.current_token.token_type)
                            {
                                self.parse_type_string()
                            } else if self.current_token.token_type == TokenType::Ident {
                                Some(self.current_token.literal.clone())
                            } else {
                                None
                            };
                            let default_value = if self.peek_token.token_type == TokenType::Assign {
                                self.next_token(); // '='
                                self.next_token(); // expr
                                Some(self.parse_expression(Precedence::Lowest)?)
                            } else {
                                None
                            };
                            if self.peek_token.token_type == TokenType::Semicolon {
                                self.next_token();
                            }
                            fields.push(ClassField {
                                name: field_name,
                                type_annotation,
                                default_value,
                                span: Span::point(field_line, field_col),
                            });
                            self.next_token();
                            continue;
                        } else if self.peek_token.token_type == TokenType::Assign {
                            // field = expr;
                            self.next_token(); // '='
                            self.next_token(); // expr
                            let default_value = Some(self.parse_expression(Precedence::Lowest)?);
                            if self.peek_token.token_type == TokenType::Semicolon {
                                self.next_token();
                            }
                            fields.push(ClassField {
                                name: field_name,
                                type_annotation: None,
                                default_value,
                                span: Span::point(field_line, field_col),
                            });
                            self.next_token();
                            continue;
                        }
                    }
                    self.had_error.set(true);
                    eprintln!(
                        "❌ PARSER ERROR: Expected 'public' or 'private' for class member, got '{}'",
                        self.current_token.literal
                    );
                    return None;
                }
            };
            self.next_token(); // after visibility

            // Check for static modifier
            let is_static = if self.current_token.token_type == TokenType::KwStatic {
                self.next_token();
                true
            } else {
                false
            };

            // Check for getter/setter
            let is_getter = if self.current_token.token_type == TokenType::KwGet {
                self.next_token();
                true
            } else {
                false
            };
            let is_setter = if !is_getter && self.current_token.token_type == TokenType::KwSet {
                self.next_token();
                true
            } else {
                false
            };

            // Optional return type keyword (void, int, decimal, [type], class name, etc.)
            let return_type = if self.current_token.token_type == TokenType::LBracket {
                // Array return type: [int], [string], [ClassName], etc.
                self.next_token(); // move to type inside brackets
                let elem = if is_type_keyword(&self.current_token.token_type) {
                    self.parse_type_string().unwrap_or_default()
                } else {
                    self.current_token.literal.clone()
                };
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array return type");
                    return None;
                }
                self.next_token(); // consume ']'
                self.next_token(); // advance to method name
                Some(format!("[{}]", elem))
            } else if is_type_keyword(&self.current_token.token_type) {
                let rt = self.parse_type_string();
                self.next_token();
                rt
            } else if self.current_token.token_type == TokenType::Ident
                && (self.peek_token.token_type == TokenType::Ident
                    || self.peek_token.token_type == TokenType::Question)
            {
                // Class return type (possibly nullable): public ClassName[?] methodName()
                let rt = self.parse_type_string();
                self.next_token();
                rt
            } else {
                None
            };

            // Member name (constructor or method) — allow keywords as names (e.g. "get", "set")
            if !self.current_token_is_name() {
                self.parser_error("Expected method name in class body");
                return None;
            }
            let member_name = self.current_token.literal.clone();

            if self.peek_token.token_type != TokenType::LParen {
                self.parser_error(&format!("Expected '(' after '{}' in class", member_name));
                return None;
            }
            self.next_token(); // '('
            let parameters = self.parse_function_parameters()?;

            // Abstract methods may have no body (semicolon) or empty body
            let body = if is_member_abstract && self.peek_token.token_type == TokenType::Semicolon {
                self.next_token(); // ';'
                BlockStatement { statements: vec![] }
            } else {
                if self.peek_token.token_type != TokenType::LBrace {
                    self.parser_error(&format!("Expected '{{' to start body of '{}'", member_name));
                    return None;
                }
                self.next_token();
                let body_stmt = self.parse_block_statement()?;
                match body_stmt {
                    Statement::Block(b) => b,
                    _ => return None,
                }
            };

            if member_name == name && !is_getter && !is_setter {
                // Constructor
                if constructor.is_some() {
                    self.parser_error(&format!("Duplicate constructor in class '{}'", name));
                    return None;
                }
                constructor = Some(ClassConstructor { parameters, body });
            } else {
                methods.push(ClassMethod {
                    name: member_name,
                    is_public: is_member_public,
                    is_abstract: is_member_abstract,
                    is_getter,
                    is_setter,
                    is_static,
                    return_type,
                    parameters,
                    body,
                });
            }

            self.next_token(); // advance past closing '}' of method/constructor
        }

        Some(Statement::ClassDeclaration(ClassDeclaration {
            name,
            is_public,
            is_abstract,
            is_sealed,
            parent,
            constructor,
            methods,
            fields,
        }))
    }

    // ── Interface declaration ─────────────────────────────────────────────────
    pub(super) fn parse_interface_declaration(&mut self, is_public: bool) -> Option<Statement> {
        // current = 'interface'
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected interface name after 'interface'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!(
                "'{}' is a reserved system namespace and cannot be used as an interface name",
                name
            ));
            return None;
        }

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after interface name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first field or '}'

        let mut fields = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("Expected field name in interface body");
                return None;
            }
            let field_name = self.current_token.literal.clone();

            if self.peek_token.token_type != TokenType::Colon {
                self.parser_error(&format!(
                    "Expected ':' after field name '{}' in interface",
                    field_name
                ));
                return None;
            }
            self.next_token(); // ':'
            self.next_token(); // type

            let type_name = if self.current_token.token_type == TokenType::LBracket {
                // Array field type: [int], [string], [ClassName], etc.
                self.next_token(); // elem type
                let elem = if is_type_keyword(&self.current_token.token_type) {
                    self.parse_type_string().unwrap_or_default()
                } else if self.current_token.token_type == TokenType::Ident {
                    self.current_token.literal.clone()
                } else {
                    self.parser_error(&format!(
                        "Expected element type inside '[...]' for field '{}' in interface",
                        field_name
                    ));
                    return None;
                };
                if self.peek_token.token_type != TokenType::RBracket {
                    self.parser_error("Expected ']' after array field type");
                    return None;
                }
                self.next_token(); // ']'
                format!("[{}]", elem)
            } else if is_type_keyword(&self.current_token.token_type) {
                match self.parse_type_string() {
                    Some(t) => t,
                    None => {
                        self.parser_error(&format!(
                            "Expected type after ':' for field '{}' in interface",
                            field_name
                        ));
                        return None;
                    }
                }
            } else if self.current_token.token_type == TokenType::Ident {
                // Class/interface type name (possibly nullable)
                self.parse_type_string()
                    .unwrap_or_else(|| self.current_token.literal.clone())
            } else {
                self.parser_error(&format!(
                    "Expected type after ':' for field '{}' in interface",
                    field_name
                ));
                return None;
            };
            fields.push(InterfaceField {
                name: field_name,
                type_name,
            });

            // consume ';' or ','
            if self.peek_token.token_type == TokenType::Semicolon
                || self.peek_token.token_type == TokenType::Comma
            {
                self.next_token();
            }
            self.next_token(); // next field or '}'
        }

        Some(Statement::InterfaceDeclaration(InterfaceDeclaration {
            name,
            is_public,
            fields,
        }))
    }

    // ── enum declaration ──────────────────────────────────────────────────────
    pub(super) fn parse_enum_declaration(&mut self) -> Option<Statement> {
        // current = 'enum'
        let line = self.current_token.line;
        let column = self.current_token.column;
        if self.peek_token.token_type != TokenType::Ident {
            self.parser_error("Expected enum name after 'enum'");
            return None;
        }
        self.next_token();
        let name = self.current_token.literal.clone();
        if self.is_reserved_name(&name) {
            self.parser_error(&format!(
                "'{}' is a reserved system namespace and cannot be used as an enum name",
                name
            ));
            return None;
        }

        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("Expected '{{' after enum name");
            return None;
        }
        self.next_token(); // '{'
        self.next_token(); // first variant or '}'

        let mut variants = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error(&format!(
                    "Expected variant name in enum body, got '{}'",
                    self.current_token.literal
                ));
                return None;
            }
            variants.push(self.current_token.literal.clone());
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // ','
                if self.peek_token.token_type == TokenType::RBrace {
                    self.next_token();
                    break;
                }
                self.next_token(); // next variant
            } else if self.peek_token.token_type == TokenType::RBrace {
                self.next_token();
                break;
            } else {
                self.parser_error("Expected ',' or '}}' in enum body");
                return None;
            }
        }

        Some(Statement::EnumDeclaration(EnumDeclaration {
            name,
            variants,
            span: Span::point(line, column),
        }))
    }

    // ── Visibility prefix (public/private class|interface) ────────────────────
    pub(super) fn parse_visibility_statement(&mut self) -> Option<Statement> {
        let is_public = self.current_token.token_type == TokenType::KwPublic;
        match self.peek_token.token_type {
            TokenType::KwClass => {
                self.next_token();
                self.parse_class_declaration(is_public, false, false)
            }
            TokenType::KwInterface => {
                self.next_token();
                self.parse_interface_declaration(is_public)
            }
            TokenType::KwAbstract => {
                self.next_token(); // 'abstract'
                if self.peek_token.token_type == TokenType::KwClass {
                    self.next_token(); // 'class'
                    self.parse_class_declaration(is_public, true, false)
                } else {
                    self.parser_error("Expected 'class' after 'abstract'");
                    None
                }
            }
            TokenType::KwSealed => {
                self.next_token(); // 'sealed'
                if self.peek_token.token_type == TokenType::KwClass {
                    self.next_token(); // 'class'
                    self.parse_class_declaration(is_public, false, true)
                } else {
                    self.parser_error("Expected 'class' after 'sealed'");
                    None
                }
            }
            _ => {
                self.had_error.set(true);
                eprintln!(
                    "❌ PARSER ERROR: Expected 'class' or 'interface' after visibility modifier"
                );
                None
            }
        }
    }

    // ── abstract class / sealed class ────────────────────────────────────────
    pub(super) fn parse_abstract_or_sealed_class(
        &mut self,
        is_abstract: bool,
        is_sealed: bool,
    ) -> Option<Statement> {
        // current = 'abstract' or 'sealed'
        if self.peek_token.token_type == TokenType::KwClass {
            self.next_token(); // 'class'
            self.parse_class_declaration(true, is_abstract, is_sealed)
        } else if self.peek_token.token_type == TokenType::KwPublic
            || self.peek_token.token_type == TokenType::KwPrivate
        {
            // public abstract class / private abstract class
            self.next_token(); // pub/priv
            if self.peek_token.token_type == TokenType::KwClass {
                self.next_token(); // 'class'
                self.parse_class_declaration(true, is_abstract, is_sealed)
            } else {
                self.parser_error("Expected 'class' after abstract/sealed");
                None
            }
        } else {
            self.parser_error("Expected 'class' after abstract/sealed");
            None
        }
    }
}
