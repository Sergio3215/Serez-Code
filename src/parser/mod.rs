//! The parser: source tokens in, a syntax tree out.
//!
//! This file is the façade. It owns the parser's state, the statement loop that
//! drives it, the dispatcher that decides which grammar a statement belongs to,
//! and the five statement forms too small to live anywhere else. Everything else
//! is one of the modules below, each extending `Parser` through its own `impl`
//! block — the pattern `src/evaluator/` already uses across 28 files.
//!
//! What is where:
//!
//! | module | responsibility |
//! |---|---|
//! | `cursor` | advancing the stream, precedence at the cursor, name classification |
//! | `depth` | `MAX_PARSE_DEPTH`, `SZ2001`, the charge/release accounting |
//! | `diagnostics` | `ParseError`, `SZ2000`, reporting and rendering |
//! | `types` | type *syntax* — never type compatibility |
//! | `directives` | `import`, `export`, `use permissions` |
//! | `variables` | `let`, `const`, destructuring patterns |
//! | `functions` | `fn`, `fn*`, parameters, `native fn`, arrows, lambdas |
//! | `classes` | `class`, `interface`, `enum`, modifier prefixes |
//! | `loops` | `while`, `do-while`, `for`, `for-in`, labels |
//! | `branches` | `if`, `switch`, `match`, `try` |
//! | `literals` | arrays, dicts, brace forms, `dec`, interpolation |
//! | `assignment` | assignment forms and receiver-writeback shape |
//! | `expressions` | the precedence table, prefix dispatcher and infix loop |
//!
//! The dependency direction is one way: the modules call back into `cursor`,
//! `depth` and `diagnostics`, and into each other only where the grammar itself
//! nests. Nothing below this file knows about the evaluator, and nothing here
//! performs semantic validation — a parse says whether source is *syntactically*
//! valid and nothing more. See `docs/maturity/ROADMAP_STATE.md` §9.

mod assignment;
mod branches;
mod classes;
mod cursor;
mod depth;
mod diagnostics;
mod directives;
mod expressions;
mod functions;
mod literals;
mod loops;
mod types;
mod variables;

use crate::span::Span;
use depth::DepthGuard;
#[allow(unused_imports)]
pub use expressions::{Precedence, token_precedence};

// Re-exported so `serez_code::parser::MAX_PARSE_DEPTH` keeps meaning what it
// meant when the constant lived in this file: `tests/frontend_robustness.rs`
// and `tests/parser_facade.rs` both name that path.
//
// The `allow` is not cosmetic. `src/lsp_main.rs` declares `mod parser;` of its
// own, so this file is compiled a second time as part of the `sz-lsp` binary —
// where `parser` is a private module of a binary crate and the re-export really
// does reach nobody. One target needs it and the other cannot use it; see
// ROADMAP_STATE.md §5.18.
#[allow(unused_imports)]
pub use depth::{MAX_PARSE_DEPTH, SZ_PARSE_DEPTH_EXCEEDED};
#[allow(unused_imports)]
pub use diagnostics::{ParseError, SZ_PARSE_ERROR};

use crate::ast::*;
use crate::lexer::{LexError, Lexer};
use crate::token::{Token, TokenType};
pub struct Parser {
    lexer: Lexer,
    current_token: Token,
    peek_token: Token,
    source_lines: Vec<String>,
    /// File (or module) the source came from — prefixes error messages so
    /// errors inside imports are attributable to their file.
    source_name: Option<String>,
    /// Set whenever any parse error is reported. `Cell` so `parser_error(&self)`
    /// can flip it. Lets callers (main) fail with a non-zero exit code.
    had_error: std::cell::Cell<bool>,
    /// Every error reported via `parser_error`, in order. `RefCell` for the
    /// same reason as `had_error`.
    errors: std::cell::RefCell<Vec<ParseError>>,
    /// Diagnostics produced while the owned lexer advances. They are flushed
    /// into `errors` once parsing finishes, after source labels/lines are set.
    lexer_errors: std::cell::RefCell<Vec<LexError>>,
    /// Current recursive-descent nesting level. See [`MAX_PARSE_DEPTH`].
    depth: std::rc::Rc<std::cell::Cell<usize>>,
}

impl Parser {
    pub fn new(mut lexer: Lexer) -> Parser {
        let current_token = lexer.next_token();
        let peek_token = lexer.next_token();
        let lexer_errors = lexer.take_errors();
        Parser {
            lexer,
            current_token,
            peek_token,
            source_lines: Vec::new(),
            source_name: None,
            had_error: std::cell::Cell::new(false),
            errors: std::cell::RefCell::new(Vec::new()),
            lexer_errors: std::cell::RefCell::new(lexer_errors),
            depth: std::rc::Rc::new(std::cell::Cell::new(0)),
        }
    }

    pub fn parse_program(&mut self) -> Program {
        let mut program = Program {
            statements: Vec::new(),
        };

        while self.current_token.token_type != TokenType::Eof {
            // Stray ';' is an empty statement (e.g. after `return;` inside a
            // one-line block, or `unsafe { ... };`), not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            match self.parse_statement() {
                Some(stmt) => program.statements.push(stmt),
                None => self.synchronize(),
            }
            self.next_token();
        }
        self.flush_lexer_errors();
        program
    }

    fn synchronize(&mut self) {
        while self.current_token.token_type != TokenType::Eof {
            match self.current_token.token_type {
                TokenType::Semicolon | TokenType::RBrace => return,
                TokenType::Let
                | TokenType::Return
                | TokenType::Out
                | TokenType::Function
                | TokenType::While
                | TokenType::For
                | TokenType::KwClass
                | TokenType::KwInterface
                | TokenType::KwPublic
                | TokenType::KwPrivate
                | TokenType::KwBreak
                | TokenType::KwContinue
                | TokenType::KwSwitch
                | TokenType::KwTry
                | TokenType::KwThrow
                | TokenType::KwConst
                | TokenType::KwEnum
                | TokenType::KwAbstract
                | TokenType::KwSealed
                | TokenType::KwDo => return,
                _ => self.next_token(),
            }
        }
    }

    fn parse_statement(&mut self) -> Option<Statement> {
        // Nested blocks recurse through here without ever building an
        // expression, so statements need their own accounting.
        let _depth = self.enter_depth()?;
        match self.current_token.token_type {
            TokenType::Let | TokenType::KwConst => self.parse_let_statement(),
            TokenType::Return => self.parse_return_statement(),
            TokenType::Out => self.parse_out_statement(),
            TokenType::LBrace => self.parse_block_statement(),
            TokenType::Function => self.parse_function_statement(),
            TokenType::While => self.parse_while_statement(),
            TokenType::KwDo => self.parse_do_while_statement(),
            TokenType::For => self.parse_for_statement(),
            TokenType::KwBreak => {
                if self.peek_token.token_type == TokenType::Ident {
                    self.next_token(); // current = label name
                    let label = self.current_token.literal.clone();
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::BreakLabel(label))
                } else {
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::Break)
                }
            }
            TokenType::KwContinue => {
                if self.peek_token.token_type == TokenType::Ident {
                    self.next_token(); // current = label name
                    let label = self.current_token.literal.clone();
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::ContinueLabel(label))
                } else {
                    if self.peek_token.token_type == TokenType::Semicolon {
                        self.next_token();
                    }
                    Some(Statement::Continue)
                }
            }
            TokenType::KwEnum => self.parse_enum_declaration(),
            TokenType::KwClass => self.parse_class_declaration(true, false, false),
            TokenType::KwInterface => self.parse_interface_declaration(true),
            TokenType::KwPublic | TokenType::KwPrivate => self.parse_visibility_statement(),
            TokenType::KwAbstract => self.parse_abstract_or_sealed_class(true, false),
            TokenType::KwSealed => self.parse_abstract_or_sealed_class(false, true),
            TokenType::KwSwitch => self.parse_switch_statement(),
            TokenType::KwTry => self.parse_try_statement(),
            TokenType::KwThrow => self.parse_throw_statement(),
            TokenType::KwUnsafe => self.parse_unsafe_statement(),
            TokenType::KwNative => self.parse_native_declaration(),
            TokenType::KwImport => self.parse_import_statement(),
            TokenType::KwExport => self.parse_export_statement(),
            TokenType::KwUse => self.parse_use_permissions(),
            TokenType::KwYield => {
                self.next_token(); // consume 'yield', current = first token of expr
                let expr = self.parse_expression(Precedence::Lowest)?;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Yield(expr))
            }
            // Labeled loop: label: while/for { ... }
            TokenType::Ident if self.peek_token.token_type == TokenType::Colon => {
                self.parse_labeled_statement()
            }
            TokenType::Ident if self.peek_token.token_type == TokenType::Assign => {
                self.parse_assign_statement()
            }
            TokenType::Ident if self.is_compound_assign(&self.peek_token.token_type) => {
                self.parse_compound_assign_statement()
            }
            TokenType::Ident if self.peek_token.token_type == TokenType::LBracket => {
                self.parse_index_assign_or_expr_statement()
            }
            // Postfix: i++  →  i = i + 1
            TokenType::Ident if self.peek_token.token_type == TokenType::PlusPlus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                self.next_token(); // '++'
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        span: Span::point(line, col),
                    }),
                    span: Span::point(line, col),
                }))
            }
            // Postfix: i--  →  i = i - 1
            TokenType::Ident if self.peek_token.token_type == TokenType::MinusMinus => {
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                self.next_token(); // '--'
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        span: Span::point(line, col),
                    }),
                    span: Span::point(line, col),
                }))
            }
            // Prefix: ++i  →  i = i + 1
            TokenType::PlusPlus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "+".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        span: Span::point(line, col),
                    }),
                    span: Span::point(line, col),
                }))
            }
            // Prefix: --i  →  i = i - 1
            TokenType::MinusMinus => {
                self.next_token(); // current = identifier
                let name = self.current_token.literal.clone();
                let line = self.current_token.line;
                let col = self.current_token.column;
                if self.peek_token.token_type == TokenType::Semicolon {
                    self.next_token();
                }
                Some(Statement::Assign(AssignStatement {
                    name: name.clone(),
                    value: Expression::Infix(InfixExpression {
                        left: Box::new(Expression::Identifier(name)),
                        operator: "-".to_string(),
                        right: Box::new(Expression::Integer(1)),
                        span: Span::point(line, col),
                    }),
                    span: Span::point(line, col),
                }))
            }
            _ => self.parse_expression_statement(),
        }
    }

    fn parse_block_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        self.next_token();
        let mut statements = Vec::new();

        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Stray ';' is an empty statement, not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }

        Some(Statement::Block(BlockStatement {
            statements,
            span: self.span_to_here(open),
        }))
    }

    fn parse_unsafe_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        if self.peek_token.token_type != TokenType::LBrace {
            self.had_error.set(true);
            eprintln!("❌ PARSE ERROR: expected '{{' after 'unsafe'");
            return None;
        }
        self.next_token(); // current = '{'
        self.next_token(); // skip '{'
        let mut statements = Vec::new();
        while self.current_token.token_type != TokenType::RBrace
            && self.current_token.token_type != TokenType::Eof
        {
            // Stray ';' is an empty statement, not an error.
            if self.current_token.token_type == TokenType::Semicolon {
                self.next_token();
                continue;
            }
            if let Some(stmt) = self.parse_statement() {
                statements.push(stmt);
            }
            self.next_token();
        }
        Some(Statement::Unsafe(BlockStatement {
            statements,
            span: self.span_to_here(open),
        }))
    }

    fn parse_return_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        // Bare `return` followed by `}`, `;`, or EOF — return null without consuming the delimiter
        if matches!(
            self.peek_token.token_type,
            TokenType::Semicolon | TokenType::RBrace | TokenType::Eof
        ) {
            return Some(Statement::Return(ReturnStatement {
                return_value: Expression::Null,
                span: self.span_to_here(open),
            }));
        }

        self.next_token();

        // Bare `return;` — no expression, return null
        if self.current_token.token_type == TokenType::Semicolon {
            return Some(Statement::Return(ReturnStatement {
                return_value: Expression::Null,
                span: self.span_to_here(open),
            }));
        }

        let return_value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Return(ReturnStatement {
            return_value,
            span: self.span_to_here(open),
        }))
    }

    fn parse_out_statement(&mut self) -> Option<Statement> {
        let open = self.current_token.span;
        self.next_token();

        let value = self.parse_expression(Precedence::Lowest)?;

        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }

        Some(Statement::Out(OutStatement {
            value,
            span: self.span_to_here(open),
        }))
    }

    // ── Lambda parsing ────────────────────────────────────────────────────────

    // ── Expression parsing ────────────────────────────────────────────────────

    // ── match expr { pattern => body, ... } ──────────────────────────────────

    // ── throw expr; ───────────────────────────────────────────────────────────
    fn parse_throw_statement(&mut self) -> Option<Statement> {
        self.next_token(); // first token of expr
        let expr = self.parse_expression(Precedence::Lowest)?;
        if self.peek_token.token_type == TokenType::Semicolon {
            self.next_token();
        }
        Some(Statement::Throw(expr))
    }
}
