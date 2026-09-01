//! Directives: the statements a file makes *about itself* rather than about a
//! value.
//!
//! Three of them, and they are together because they answer one question — what
//! does this file need from, and offer to, the world outside it:
//!
//!   * `import "path"` — what it pulls in (`spec/modules.md`)
//!   * `export <decl>` — what it makes visible (`spec/modules.md`)
//!   * `use permissions { … }` — what capabilities it claims (`spec/security.md`)
//!
//! All three are top-level and none of them produces a value. That is the shared
//! reason to change: a decision about the module system or the permission
//! vocabulary lands here, and nothing about expressions or control flow does.
//!
//! Two notes worth carrying, both of which are the *runtime's* problem and not
//! this file's:
//!
//!   * `export` is a wrapper. It parses the declaration that follows and hands
//!     it back inside `Statement::Export`, so every rule about *what* may be
//!     exported lives in the evaluator, not in the grammar.
//!   * `use permissions` parses dotted names (`OS.exec`, `File.delete`) but
//!     validates nothing. An unknown permission is syntactically fine here;
//!     `spec/security.md` records that permissions are additive declarations
//!     and not an isolation boundary.

use super::Parser;
use crate::ast::Statement;
use crate::token::TokenType;

impl Parser {
    pub(super) fn parse_export_statement(&mut self) -> Option<Statement> {
        // export <declaration>  —  wraps any top-level declaration
        self.next_token(); // consume 'export', move to the inner keyword
        let inner = self.parse_statement()?;
        Some(Statement::Export(Box::new(inner)))
    }

    pub(super) fn parse_use_permissions(&mut self) -> Option<Statement> {
        // use permissions { Terminal, OS.exec, File.delete }
        if self.peek_token.token_type != TokenType::Ident
            || self.peek_token.literal != "permissions"
        {
            self.parser_error("expected 'permissions' after 'use'");
            return None;
        }
        self.next_token(); // current = "permissions"
        if self.peek_token.token_type != TokenType::LBrace {
            self.parser_error("expected '{' after 'use permissions'");
            return None;
        }
        self.next_token(); // current = '{'
        let mut perms: Vec<String> = Vec::new();
        loop {
            if self.peek_token.token_type == TokenType::RBrace
                || self.peek_token.token_type == TokenType::Eof
            {
                self.next_token();
                break;
            }
            self.next_token(); // current = permission name (Ident)
            if self.current_token.token_type != TokenType::Ident {
                self.parser_error("expected permission name inside 'use permissions { }'");
                return None;
            }
            let mut perm = self.current_token.literal.clone();
            // Handle dotted names: OS.exec, File.delete
            while self.peek_token.token_type == TokenType::Dot {
                self.next_token(); // current = '.'
                if self.peek_token.token_type != TokenType::Ident {
                    self.parser_error("expected identifier after '.' in permission name");
                    return None;
                }
                self.next_token(); // current = sub-name
                perm.push('.');
                perm.push_str(&self.current_token.literal);
            }
            perms.push(perm);
            if self.peek_token.token_type == TokenType::Comma {
                self.next_token(); // consume ','
            } else if self.peek_token.token_type == TokenType::RBrace {
                self.next_token(); // consume '}'
                break;
            } else {
                self.parser_error("expected ',' or '}' in 'use permissions'");
                return None;
            }
        }
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::UsePermissions(perms))
    }

    pub(super) fn parse_import_statement(&mut self) -> Option<Statement> {
        // import "path/to/module";
        if self.peek_token.token_type != TokenType::String {
            self.parser_error("expected string path after 'import'");
            return None;
        }
        self.next_token(); // current = string literal
        let path = self.current_token.literal.clone();
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Import(path))
    }
}
